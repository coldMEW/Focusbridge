import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.serialization)
    alias(libs.plugins.ksp)
    alias(libs.plugins.hilt)
    alias(libs.plugins.google.services)
}

/**
 * Release signing credentials, when they are available on this machine.
 *
 * Looked for at `FOCUSBRIDGE_KEYSTORE_PROPERTIES`, or beside the checkout at
 * `../../FocusBridge-signing/keystore.properties`. Absent, the build still works
 * and signs with the debug key; present, it produces something installable by
 * real users. Losing this keystore means never being able to update the app, so
 * it lives outside the repository and outside anything that gets shared.
 */
val releaseSigning: Properties? = run {
    val fromEnv = System.getenv("FOCUSBRIDGE_KEYSTORE_PROPERTIES")?.let { file(it) }
    val beside = rootProject.file("../../FocusBridge-signing/keystore.properties")
    val source = listOfNotNull(fromEnv, beside).firstOrNull { it.exists() } ?: return@run null
    Properties().apply { source.inputStream().use { load(it) } }
}

android {
    namespace = "com.focusbridge.android"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.focusbridge.android"
        minSdk = 26
        targetSdk = 34
        versionCode = 1
        versionName = "1.0.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    testOptions.unitTests.isReturnDefaultValues = true

    composeOptions {
        kotlinCompilerExtensionVersion = libs.versions.composeCompiler.get()
    }

    signingConfigs {
        // Release signing material never lives in the repository. The properties
        // file points at a keystore kept outside it, and the build falls back to
        // the debug key when it is absent, so a checkout still builds -- but a
        // release built that way is only ever for testing, because the debug key
        // is shared by every Android SDK on earth.
        if (releaseSigning != null) {
            create("release") {
                storeFile = file(releaseSigning.getProperty("storeFile"))
                storePassword = releaseSigning.getProperty("storePassword")
                keyAlias = releaseSigning.getProperty("keyAlias")
                keyPassword = releaseSigning.getProperty("keyPassword")
                // v2 verifies the whole archive; v3 adds the ability to rotate
                // this key later, which matters because a signing key that can
                // never change is a signing key that can never recover.
                enableV2Signing = true
                enableV3Signing = true
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            signingConfig = if (releaseSigning != null) {
                signingConfigs.getByName("release")
            } else {
                signingConfigs.getByName("debug")
            }
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }
}

dependencies {
    implementation(libs.core.ktx)
    implementation(libs.lifecycle.runtime.ktx)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.activity.compose)
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.compose.material3)
    implementation(libs.compose.material.icons.extended)
    implementation(libs.navigation.compose)
    implementation(libs.hilt.android)
    ksp(libs.hilt.compiler)
    implementation(libs.hilt.navigation.compose)
    implementation(libs.room.runtime)
    implementation(libs.room.ktx)
    // Use the maintained Android binding, not deprecated android-database-sqlcipher.
    // Explicit AAR integration retains Room 2.6.1's SQLite support API and Kotlin 1.9 toolchain.
    implementation("net.zetetic:sqlcipher-android:4.17.0@aar")
    implementation("androidx.sqlite:sqlite:2.4.0")
    ksp(libs.room.compiler)
    implementation(libs.okhttp)
    implementation(libs.okhttp.logging)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.coroutines.android)
    implementation(libs.zxing.core)
    implementation(libs.camerax.camera2)
    implementation(libs.camerax.lifecycle)
    implementation(libs.camerax.view)
    implementation(platform(libs.firebase.bom))
    implementation(libs.firebase.auth)
    debugImplementation(libs.compose.ui.tooling)
    testImplementation(libs.junit)
    testImplementation(libs.mockk)
    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.espresso.core)
    androidTestImplementation(platform(libs.compose.bom))
}
