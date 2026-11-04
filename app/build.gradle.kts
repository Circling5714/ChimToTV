import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

// Khoá ký bản release: keys/ky.properties (build.ps1 tự tạo lần đầu). Thiếu thì ký bằng khoá debug.
val kyFile = rootProject.file("keys/ky.properties")
val ky = Properties().apply { if (kyFile.exists()) kyFile.inputStream().use { load(it) } }

android {
    namespace = "vn.chimto.tv"
    compileSdk = 35

    defaultConfig {
        applicationId = "vn.chimto.tv"
        // Android 5.0 trở lên: chạy được cả trên các TV box đời cũ.
        minSdk = 21
        targetSdk = 35
        versionCode = 15
        versionName = "1.9.2"
        // Thử cập nhật OTA với bản giả số hiệu cao hơn: gradlew assembleDebug -Pvc=5 -Pvn=1.4
        (findProperty("vc") as String?)?.let { versionCode = it.toInt() }
        (findProperty("vn") as String?)?.let { versionName = it }

        // Nơi app hỏi bản phát hành mới nhất (GitHub Releases của repo)
        buildConfigField("String", "UPDATE_URL", "\"https://api.github.com/repos/Circling5714/ChimToTV/releases/latest\"")
    }
    signingConfigs {
        if (kyFile.exists()) {
            create("chimto") {
                storeFile = rootProject.file(ky.getProperty("storeFile"))
                storePassword = ky.getProperty("storePassword")
                keyAlias = ky.getProperty("keyAlias")
                keyPassword = ky.getProperty("keyPassword")
            }
        }
    }
    buildFeatures { buildConfig = true }
    buildTypes {
        // Bản gỡ lỗi cài song song với bản thật (vn.chimto.tv.debug), bật gỡ lỗi WebView để đo / soi qua DevTools
        debug {
            applicationIdSuffix = ".debug"
            versionNameSuffix = "-debug"
            // Bản gỡ lỗi hỏi máy chủ giả trên máy tính (adb reverse tcp:8765 tcp:8765) để thử OTA mà không phát hành thật
            buildConfigField("String", "UPDATE_URL", "\"http://127.0.0.1:8765/latest.json\"")
        }
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("chimto") ?: signingConfigs.getByName("debug")
        }
    }
    compileOptions {
        // jsoup dùng API Java 8 (java.util.function…) không có sẵn dưới Android 7.
        isCoreLibraryDesugaringEnabled = true
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    dependenciesInfo { includeInApk = false; includeInBundle = false }
    // Test JVM gọi Site.parse* — các lớp Android (LruCache…) trả về giá trị mặc định
    testOptions { unitTests.isReturnDefaultValues = true }
}

dependencies {
    implementation("androidx.recyclerview:recyclerview:1.3.2")
    implementation("org.jsoup:jsoup:1.18.1")
    // ProxyController: cho WebView đi qua proxy SOCKS5 (1.14 là bản cuối còn chạy Android 5; 1.15+ cần Android 6+)
    implementation("androidx.webkit:webkit:1.14.0")
    coreLibraryDesugaring("com.android.tools:desugar_jdk_libs:2.1.2")
    testImplementation("junit:junit:4.13.2")
    // org.json của Android trong test JVM chỉ là vỏ rỗng — NextSite đọc JSON nên test cần bản thật
    testImplementation("org.json:json:20240303")
}
