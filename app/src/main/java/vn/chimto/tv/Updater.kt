package vn.chimto.tv

import android.app.Activity
import android.app.AlertDialog
import android.app.PendingIntent
import android.content.ActivityNotFoundException
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInfo
import android.content.pm.PackageInstaller
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.util.Log
import android.widget.Toast
import org.json.JSONObject
import java.io.File
import java.io.IOException
import java.security.MessageDigest

/**
 * Cập nhật OTA từ GitHub Releases: hỏi bản phát hành mới nhất → tải APK ngầm → kiểm tra cùng gói, cùng chữ ký,
 * số hiệu lớn hơn → cài qua PackageInstaller. Android luôn hỏi xác nhận một lần (trừ Android 12+ khi chính app
 * là bên cài bản đang chạy — lúc đó cập nhật được hoàn toàn tự động).
 *
 * Phát hành bản mới: tạo release tag "vX.Y" trên GitHub, đính kèm tệp .apk (build.ps1 tạo sẵn).
 */
object Updater {
    private const val CHECK_EVERY = 2 * 60 * 60 * 1000L
    private const val SNOOZE = 24 * 60 * 60 * 1000L

    data class Release(val version: String, val apkUrl: String, val notes: String)

    /** Bản đã tải và kiểm tra xong, chờ người xem đồng ý cài. */
    @Volatile private var ready: Pair<Release, File>? = null
    @Volatile private var busy = false

    /** Bản phát hành mới nhất (GitHub tự bỏ qua bản nháp / thử nghiệm). */
    fun latest(): Release? {
        val r = Http.get(BuildConfig.UPDATE_URL, mapOf("Accept" to "application/vnd.github+json"))
        val o = JSONObject(r.body)
        val version = o.optString("tag_name").trim().removePrefix("v").removePrefix("V")
        val assets = o.optJSONArray("assets") ?: return null
        val apk = (0 until assets.length()).map { assets.getJSONObject(it) }
            .firstOrNull { it.optString("name").endsWith(".apk", true) } ?: return null
        if (version.isEmpty()) return null
        return Release(version, apk.getString("browser_download_url"), o.optString("body"))
    }

    /** So số hiệu dạng 1.2.3 (bỏ hậu tố như "-debug"). */
    fun isNewer(remote: String, local: String = BuildConfig.VERSION_NAME): Boolean {
        val a = parts(remote)
        val b = parts(local)
        for (i in 0 until maxOf(a.size, b.size)) {
            val x = a.getOrElse(i) { 0 }
            val y = b.getOrElse(i) { 0 }
            if (x != y) return x > y
        }
        return false
    }

    private fun parts(v: String) = v.substringBefore('-').split('.').map { it.trim().toIntOrNull() ?: 0 }

    /** Tải APK; mạng TV hay chập chờn nên đứt giữa chừng thì thử lại (tối đa 3 lần). */
    fun download(context: Context, rel: Release): File {
        val dir = File(context.cacheDir, "update").apply { mkdirs() }
        dir.listFiles()?.forEach { it.delete() }
        val file = File(dir, "ChimToTV-${rel.version}.apk")
        var lastError: IOException? = null
        repeat(3) { attempt ->
            val c = Http.open(rel.apkUrl)
            try {
                if (c.responseCode != 200) throw IOException("Tải bản cập nhật lỗi HTTP ${c.responseCode}")
                c.inputStream.use { input -> file.outputStream().use { input.copyTo(it) } }
                return file
            } catch (e: IOException) {
                lastError = e
                Log.w("ChimToUpdate", "Tải lần ${attempt + 1} lỗi: ${e.message}")
            } finally {
                c.disconnect()
            }
        }
        file.delete()
        throw lastError ?: IOException("Không tải được bản cập nhật")
    }

    /** null nếu APK dùng được; ngược lại là lý do từ chối. */
    fun verify(context: Context, apk: File): String? {
        val pm = context.packageManager
        // Android 9 không điền signingInfo khi đọc tệp APK chưa cài → xin cả kiểu cũ (signatures) để dự phòng
        @Suppress("DEPRECATION")
        val flags = PackageManager.GET_SIGNATURES or if (Build.VERSION.SDK_INT >= 28) PackageManager.GET_SIGNING_CERTIFICATES else 0
        val archive = pm.getPackageArchiveInfo(apk.path, flags) ?: return "Tệp tải về không phải APK hợp lệ"
        if (archive.packageName != context.packageName) return "APK không phải của ChimTo TV"
        val mine = pm.getPackageInfo(context.packageName, flags)
        val theirs = signers(archive)
        if (theirs.isEmpty() || theirs != signers(mine)) return "Chữ ký APK không khớp với bản đang cài — bỏ qua"
        if (versionCode(archive) <= versionCode(mine)) return "Bản tải về không mới hơn bản đang cài"
        return null
    }

    @Suppress("DEPRECATION")
    private fun signers(pi: PackageInfo): Set<String> {
        val modern = if (Build.VERSION.SDK_INT >= 28) pi.signingInfo?.apkContentsSigners else null
        val sigs = modern?.takeIf { it.isNotEmpty() } ?: pi.signatures
        return sigs.orEmpty().map { s ->
            MessageDigest.getInstance("SHA-256").digest(s.toByteArray()).joinToString("") { "%02x".format(it) }
        }.toSet()
    }

    @Suppress("DEPRECATION")
    private fun versionCode(pi: PackageInfo): Long =
        if (Build.VERSION.SDK_INT >= 28) pi.longVersionCode else pi.versionCode.toLong()

    /** Android 8+: app cần được phép "cài ứng dụng không rõ nguồn". */
    private fun canInstall(context: Context) =
        Build.VERSION.SDK_INT < 26 || context.packageManager.canRequestPackageInstalls()

    fun install(context: Context, apk: File) {
        val installer = context.packageManager.packageInstaller
        val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL).apply {
            setAppPackageName(context.packageName)
            // Android 12+: không cần bấm xác nhận nếu bản đang chạy do chính app cài
            if (Build.VERSION.SDK_INT >= 31) setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED)
        }
        val id = installer.createSession(params)
        installer.openSession(id).use { session ->
            session.openWrite("base.apk", 0, apk.length()).use { out ->
                apk.inputStream().use { it.copyTo(out) }
                session.fsync(out)
            }
            val flags = PendingIntent.FLAG_UPDATE_CURRENT or if (Build.VERSION.SDK_INT >= 31) PendingIntent.FLAG_MUTABLE else 0
            val status = PendingIntent.getBroadcast(context, id, Intent(context, InstallReceiver::class.java), flags)
            session.commit(status.intentSender)
        }
    }

    // ---- Giao diện ----

    /** Gọi ở onResume của màn hình chính: tối đa 2 giờ / lần, có bản mới thì tải ngầm rồi hỏi cài. */
    fun autoCheck(activity: Activity) {
        if (BuildConfig.UPDATE_URL.isEmpty() || !Prefs.autoUpdate) return
        ready?.let { (rel, file) ->
            if (!Prefs.isUpdateSnoozed(rel.version)) offer(activity, rel, file)
            return
        }
        if (busy || System.currentTimeMillis() - Prefs.lastUpdateCheck < CHECK_EVERY) return
        run(activity, manual = false) {}
    }

    /** Nút "Kiểm tra cập nhật" trong Cài đặt. */
    fun checkNow(activity: Activity, status: (String) -> Unit) {
        ready?.let { (rel, file) ->
            offer(activity, rel, file)
            return
        }
        if (busy) return status("Đang kiểm tra…")
        status("Đang kiểm tra bản mới…")
        run(activity, manual = true, status)
    }

    private fun run(activity: Activity, manual: Boolean, status: (String) -> Unit) {
        busy = true
        Prefs.lastUpdateCheck = System.currentTimeMillis()
        val app = activity.applicationContext
        activity.background<Pair<Release?, File?>>({
            val rel = latest()
            if (rel == null || !isNewer(rel.version)) return@background null to null
            if (!manual && Prefs.isUpdateSnoozed(rel.version)) return@background null to null
            val file = download(app, rel)
            verify(app, file)?.let { reason ->
                file.delete()
                throw IOException(reason)
            }
            rel to file
        }) { r ->
            busy = false
            r.onSuccess { (rel, file) ->
                if (rel == null || file == null) {
                    status("Đang dùng bản mới nhất (${BuildConfig.VERSION_NAME}).")
                } else {
                    ready = rel to file
                    status("Đã tải bản ${rel.version}.")
                    offer(activity, rel, file)
                }
            }.onFailure {
                Log.w("ChimToUpdate", "Kiểm tra cập nhật lỗi", it)
                status("Không kiểm tra được cập nhật: ${it.friendly()}")
            }
        }
    }

    private fun offer(activity: Activity, rel: Release, file: File) {
        if (activity.isFinishing || activity.isDestroyed) return
        AlertDialog.Builder(activity, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle("Có bản mới ${rel.version} (đang dùng ${BuildConfig.VERSION_NAME})")
            .setMessage(summary(rel.notes).ifEmpty { "Bản cập nhật đã tải sẵn." })
            .setPositiveButton("Cài đặt ngay") { _, _ -> startInstall(activity, file) }
            .setNegativeButton("Để sau") { _, _ -> Prefs.snoozeUpdate(rel.version, System.currentTimeMillis() + SNOOZE) }
            .show()
            .getButton(AlertDialog.BUTTON_POSITIVE)?.requestFocus()
    }

    private fun startInstall(activity: Activity, file: File) {
        if (!canInstall(activity)) {
            Toast.makeText(activity, "Bật \"Cho phép cài ứng dụng\" cho ChimTo TV rồi quay lại app.", Toast.LENGTH_LONG).show()
            try {
                activity.startActivity(
                    Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${activity.packageName}")),
                )
            } catch (e: ActivityNotFoundException) {
                Toast.makeText(activity, "Vào Cài đặt tivi → Ứng dụng → Quyền đặc biệt → Cài ứng dụng không rõ nguồn → bật ChimTo TV.", Toast.LENGTH_LONG).show()
            }
            return
        }
        try {
            install(activity, file)
            Toast.makeText(activity, "Đang cài bản cập nhật…", Toast.LENGTH_SHORT).show()
        } catch (e: Exception) {
            Toast.makeText(activity, "Không cài được: ${e.friendly()}", Toast.LENGTH_LONG).show()
        }
    }

    /** Ghi chú phát hành (markdown) → vài dòng chữ thường cho hộp thoại; có mục "Có gì mới" thì chỉ lấy mục đó. */
    private fun summary(notes: String): String {
        val lines = notes.lines().map { it.trim() }
        val start = lines.indexOfFirst { it.startsWith("#") && it.contains("Có gì mới", ignoreCase = true) }
        val section = if (start >= 0) lines.drop(start + 1).takeWhile { !it.startsWith("## ") } else lines
        return section
            .filter { it.isNotEmpty() && !it.startsWith("![") && !it.startsWith("|") && !it.contains("SHA-256") }
            .map { it.trimStart('#', '-', '*', ' ').replace("**", "").replace("`", "") }
            .filter { it.isNotEmpty() }
            .joinToString("\n")
            .take(600)
    }
}

/** Kết quả phiên cài đặt: cần xác nhận thì mở hộp thoại của hệ thống; lỗi thì báo. */
class InstallReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)) {
            PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                @Suppress("DEPRECATION")
                val confirm = if (Build.VERSION.SDK_INT >= 33) {
                    intent.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java)
                } else {
                    intent.getParcelableExtra(Intent.EXTRA_INTENT)
                }
                confirm?.let { context.startActivity(it.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
            }
            PackageInstaller.STATUS_SUCCESS -> Unit // UpdatedReceiver mở lại app
            else -> Toast.makeText(
                context, "Cập nhật không thành công: ${intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE)}",
                Toast.LENGTH_LONG,
            ).show()
        }
    }
}

/** Đã cài xong bản mới: dọn tệp tải về và mở lại app (Android 10+ có thể chặn mở từ nền — khi đó mở tay). */
class UpdatedReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        File(context.cacheDir, "update").listFiles()?.forEach { it.delete() }
        try {
            context.startActivity(Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        } catch (e: Exception) {
            // không mở được từ nền: bỏ qua
        }
    }
}
