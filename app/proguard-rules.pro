# jsoup tham chiếu tới re2j (tuỳ chọn) — không dùng.
-dontwarn com.google.re2j.**
-keepattributes *Annotation*

# GeckoView (WebFetch.kt): snakeyaml đọc tệp cấu hình Gecko, có tham chiếu java.beans (Android không có, Gecko không dùng tới)
-dontwarn java.beans.**
# Mã native của Gecko gọi lớp / hàm Java theo tên (JNI) — R8 đổi tên / cắt là văng (SIGSEGV luồng «launcher» lúc khởi động)
-keep class org.mozilla.** { *; }
-keep class org.yaml.snakeyaml.** { *; }
# Chú thích Kotlin mà ExoPlayer nhúng trong Gecko tham chiếu (chỉ là chú thích, không cần lúc chạy)
-dontwarn kotlin.annotations.jvm.MigrationStatus
-dontwarn kotlin.annotations.jvm.UnderMigration
