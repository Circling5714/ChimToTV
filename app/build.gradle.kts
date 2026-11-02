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
        versionCode = 2
        versionName = "1.1"
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
    buildTypes {
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
}

dependencies {
    implementation("androidx.recyclerview:recyclerview:1.3.2")
    implementation("org.jsoup:jsoup:1.18.1")
    coreLibraryDesugaring("com.android.tools:desugar_jdk_libs:2.1.2")
}
