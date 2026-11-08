plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.seatlock.listeninglab"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.seatlock.listeninglab"
        minSdk = 26
        targetSdk = 35
        versionCode = 2
        versionName = "0.1.0"
    }

    signingConfigs {
        create("release") {
            storeFile = file(System.getenv("SEATLOCK_KEYSTORE") ?: "missing.keystore")
            storePassword = System.getenv("SEATLOCK_STORE_PASS")
            keyAlias = "seatlock"
            keyPassword = System.getenv("SEATLOCK_KEY_PASS")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("release")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.13.1")
}
