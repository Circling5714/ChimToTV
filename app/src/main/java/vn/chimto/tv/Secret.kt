package vn.chimto.tv

import android.annotation.TargetApi
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Mã hoá chuỗi bí mật (mật khẩu proxy) bằng khoá AES nằm trong Android Keystore: khoá không lấy ra được, nên chép được
 * file dữ liệu của app (root, sao lưu cũ) cũng không đọc được mật khẩu. Không chống được mã chạy ngay trong app.
 * Cần Android 6 trở lên; Android 5 thì lưu như cũ (supported = false).
 */
@TargetApi(Build.VERSION_CODES.M) // chỉ gọi khi supported (Prefs.proxyPass kiểm trước)
object Secret {
    private const val ALIAS = "chimto_secret"
    private const val IV = 12

    val supported get() = Build.VERSION.SDK_INT >= 23

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        gen.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .build()
        )
        return gen.generateKey()
    }

    /** "iv + bản mã" dạng Base64; null nếu Keystore của máy lỗi. */
    fun encrypt(plain: String): String? = runCatching {
        val c = Cipher.getInstance("AES/GCM/NoPadding")
        c.init(Cipher.ENCRYPT_MODE, key())
        Base64.encodeToString(c.iv + c.doFinal(plain.toByteArray(Charsets.UTF_8)), Base64.NO_WRAP)
    }.getOrNull()

    /** null nếu không giải được (khoá mất — vd cài lại app — hoặc dữ liệu hỏng). */
    fun decrypt(enc: String): String? = runCatching {
        val b = Base64.decode(enc, Base64.NO_WRAP)
        val c = Cipher.getInstance("AES/GCM/NoPadding")
        c.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, b, 0, IV))
        String(c.doFinal(b, IV, b.size - IV), Charsets.UTF_8)
    }.getOrNull()
}
