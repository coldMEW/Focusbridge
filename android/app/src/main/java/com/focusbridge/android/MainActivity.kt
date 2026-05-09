package com.focusbridge.android

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.os.PowerManager
import android.provider.Settings
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.compose.setContent
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Article
import androidx.compose.material.icons.filled.Block
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.CameraAlt
import androidx.compose.material.icons.filled.Devices
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.NotificationsActive
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material.icons.filled.Security
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Shield
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.Sync
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material.icons.filled.Warning
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.res.painterResource
import androidx.core.content.ContextCompat
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.lifecycleScope
import com.focusbridge.android.data.local.NotificationEntity
import com.focusbridge.android.data.local.PairingEntity
import com.focusbridge.android.data.repository.ConfigRepository
import com.focusbridge.android.data.repository.NotificationRepository
import com.focusbridge.android.data.repository.PairingRepository
import com.focusbridge.android.pairing.PairingManager
import com.focusbridge.android.security.MobileAppLockCrypto
import com.focusbridge.android.service.SyncForegroundService
import com.focusbridge.android.sync.ConnectionState
import com.focusbridge.android.sync.WebSocketClient
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import dagger.hilt.android.AndroidEntryPoint
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import javax.inject.Inject

@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    @Inject lateinit var notifications: NotificationRepository
    @Inject lateinit var pairingRepository: PairingRepository
    @Inject lateinit var pairingManager: PairingManager
    @Inject lateinit var configRepository: ConfigRepository
    @Inject lateinit var webSocketClient: WebSocketClient

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        consumePairingIntent(intent)
        setContent {
            FocusBridgeTheme {
                FocusBridgeScreen(
                    notifications = notifications,
                    pairingRepository = pairingRepository,
                    pairingManager = pairingManager,
                    configRepository = configRepository,
                    connectionState = webSocketClient.state.collectAsState().value,
                    reconnectRequest = webSocketClient.reconnectRequest.collectAsState().value,
                    openNotificationAccess = {
                        startActivity(Intent(Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS))
                    },
                    openBatterySettings = {
                        startActivity(Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS))
                    },
                    startSync = {
                        ContextCompat.startForegroundService(
                            this,
                            Intent(this, SyncForegroundService::class.java),
                        )
                    },
                    acceptReconnect = {
                        webSocketClient.acceptReconnectRequest()
                        ContextCompat.startForegroundService(
                            this,
                            Intent(this, SyncForegroundService::class.java),
                        )
                    },
                    dismissReconnect = {
                        webSocketClient.dismissReconnectRequest()
                    },
                    manualDisconnect = {
                        webSocketClient.manualDisconnect()
                    },
                )
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        consumePairingIntent(intent)
    }

    private fun consumePairingIntent(intent: Intent?) {
        val payload = intent?.data?.toString()?.takeIf { it.startsWith("focusbridge://pair") } ?: return
        lifecycleScope.launch {
            runCatching { pairingManager.consume(payload) }
                .onSuccess {
                    ContextCompat.startForegroundService(
                        this@MainActivity,
                        Intent(this@MainActivity, SyncForegroundService::class.java),
                    )
                    Toast.makeText(this@MainActivity, "FocusBridge paired. Starting sync.", Toast.LENGTH_LONG).show()
                }
                .onFailure {
                    Toast.makeText(this@MainActivity, "Pairing failed: ${it.message}", Toast.LENGTH_LONG).show()
                }
        }
    }

    companion object {
        const val EXTRA_SHOW_RECONNECT_PROMPT = "focusbridge.show_reconnect_prompt"
    }
}

private enum class AppTab(val label: String) {
    Home("Home"),
    Pair("Pair"),
    Rules("Rules"),
    Log("Log"),
}

private val MOBILE_SECURITY_QUESTIONS = listOf(
    "What city were you born in?",
    "What was the name of your first school?",
    "What was your childhood nickname?",
    "What is the name of your favorite teacher?",
    "What was the model of your first phone?",
    "Custom question",
)

@Composable
private fun FocusBridgeScreen(
    notifications: NotificationRepository,
    pairingRepository: PairingRepository,
    pairingManager: PairingManager,
    configRepository: ConfigRepository,
    connectionState: ConnectionState,
    reconnectRequest: com.focusbridge.android.sync.DesktopReconnectRequest?,
    openNotificationAccess: () -> Unit,
    openBatterySettings: () -> Unit,
    startSync: () -> Unit,
    acceptReconnect: () -> Unit,
    dismissReconnect: () -> Unit,
    manualDisconnect: () -> Unit,
) {
    val context = LocalContext.current
    val items by notifications.observeRecent().collectAsState(initial = emptyList())
    val activePairing by pairingRepository.observeActive().collectAsState(initial = null)
    val studyModeFlow = remember(configRepository) {
        configRepository.observe("study_mode_enabled").map { it == "true" }
    }
    val studyMode by studyModeFlow.collectAsState(initial = false)
    val privacyModeFlow = remember(configRepository) {
        configRepository.observe("privacy_mode_enabled").map { it == "true" }
    }
    val privacyMode by privacyModeFlow.collectAsState(initial = false)
    val priorityKeywords by configRepository.observe("priority_keywords").collectAsState(initial = "")
    val favoriteContacts by configRepository.observe("favorite_contacts").collectAsState(initial = "")
    val blockedKeywords by configRepository.observe("blocked_keywords").collectAsState(initial = "")
    val appLockEnabled by configRepository.observe("mobile_lock_enabled").map { it == "true" }.collectAsState(initial = false)
    val appLockSalt by configRepository.observe("mobile_lock_salt").collectAsState(initial = "")
    val appLockHash by configRepository.observe("mobile_lock_hash").collectAsState(initial = "")
    val recoveryQuestion by configRepository.observe("mobile_lock_recovery_question").collectAsState(initial = "")
    val recoverySalt by configRepository.observe("mobile_lock_recovery_salt").collectAsState(initial = "")
    val recoveryHash by configRepository.observe("mobile_lock_recovery_hash").collectAsState(initial = "")
    var appUnlocked by remember { mutableStateOf(false) }
    var tab by remember { mutableStateOf(AppTab.Home) }
    val scope = rememberCoroutineScope()
    val notificationPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) {}
    val lifecycleOwner = LocalLifecycleOwner.current
    var notificationAccessEnabled by remember { mutableStateOf(isNotificationAccessEnabled(context)) }
    var postNotificationsAllowed by remember { mutableStateOf(isPostNotificationsAllowed(context)) }
    var batteryUnrestricted by remember { mutableStateOf(isIgnoringBatteryOptimizations(context)) }

    DisposableEffect(lifecycleOwner, context) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                notificationAccessEnabled = isNotificationAccessEnabled(context)
                postNotificationsAllowed = isPostNotificationsAllowed(context)
                batteryUnrestricted = isIgnoringBatteryOptimizations(context)
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    LaunchedEffect(Unit) {
        if (
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            !postNotificationsAllowed
        ) {
            notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
        startSync()
    }

    LaunchedEffect(appLockEnabled) {
        if (!appLockEnabled) appUnlocked = false
    }

    if (reconnectRequest != null) {
        AlertDialog(
            onDismissRequest = dismissReconnect,
            title = { Text("Reconnect to desktop?") },
            text = {
                Text(
                    "Your desktop is asking this phone to resume FocusBridge sync. Accept only if you recognize this desktop.",
                    color = Color(0xFF61706A),
                )
            },
            confirmButton = {
                Button(
                    onClick = acceptReconnect,
                    colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF17221E)),
                ) {
                    Text("Accept and sync")
                }
            },
            dismissButton = {
                TextButton(onClick = dismissReconnect) {
                    Text("Not now")
                }
            },
        )
    }

    if (appLockEnabled && !appUnlocked && !appLockHash.isNullOrBlank()) {
        MobileAppLockGate(
            appLockSalt = appLockSalt,
            appLockHash = appLockHash,
            recoveryQuestion = recoveryQuestion,
            recoveryHash = recoveryHash,
            onUnlock = { appUnlocked = true },
            onResetSecret = { newSecret, answer, onResult ->
                scope.launch {
                    val validAnswer = MobileAppLockCrypto.verify(answer.lowercase(), recoverySalt, recoveryHash)
                    if (!validAnswer) {
                        onResult("Security answer did not match.")
                        return@launch
                    }
                    if (!MobileAppLockCrypto.validSecret(newSecret)) {
                        onResult("Use a 4+ digit PIN or 8+ character password.")
                        return@launch
                    }
                    val salt = MobileAppLockCrypto.newSalt()
                    configRepository.set("mobile_lock_salt", salt)
                    configRepository.set("mobile_lock_hash", MobileAppLockCrypto.hashSecret(newSecret, salt))
                    appUnlocked = true
                    onResult("Local app lock reset.")
                }
            },
        )
        return
    }

    Surface(
        modifier = Modifier.fillMaxSize(),
        color = Color(0xFFF5EFE4),
    ) {
        BoxWithConstraints(
            modifier = Modifier
                .fillMaxSize()
                .background(
                    Brush.verticalGradient(
                        colors = listOf(Color(0xFFF9F2E7), Color(0xFFE1ECE3)),
                    ),
                ),
        ) {
            val compact = maxWidth < 380.dp || maxHeight < 720.dp
            val screenPadding = if (compact) 10.dp else 18.dp
            val cardGap = if (compact) 10.dp else 16.dp

            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(screenPadding),
            ) {
                Header(connectionState = connectionState, studyMode = studyMode, compact = compact) { on ->
                    scope.launch { configRepository.set("study_mode_enabled", on.toString()) }
                }
                Spacer(Modifier.height(cardGap))

                Box(modifier = Modifier.weight(1f)) {
                    when (tab) {
                        AppTab.Home -> HomeTab(
                            items = items,
                            activePairing = activePairing,
                            connectionState = connectionState,
                            notificationAccessEnabled = notificationAccessEnabled,
                            postNotificationsAllowed = postNotificationsAllowed,
                            batteryUnrestricted = batteryUnrestricted,
                            compact = compact,
                            openNotificationAccess = openNotificationAccess,
                            openBatterySettings = openBatterySettings,
                            startSync = startSync,
                            manualDisconnect = manualDisconnect,
                        )
                        AppTab.Pair -> PairTab(pairingManager = pairingManager, startSync = startSync, compact = compact)
                        AppTab.Rules -> RulesTab(
                            studyMode = studyMode,
                            privacyMode = privacyMode,
                            priorityKeywords = priorityKeywords.orEmpty(),
                            favoriteContacts = favoriteContacts.orEmpty(),
                            blockedKeywords = blockedKeywords.orEmpty(),
                            onStudyModeChange = { on ->
                                scope.launch { configRepository.set("study_mode_enabled", on.toString()) }
                            },
                            onPrivacyModeChange = { on ->
                                scope.launch { configRepository.set("privacy_mode_enabled", on.toString()) }
                            },
                            onPriorityKeywordsChange = { value ->
                                scope.launch { configRepository.set("priority_keywords", value) }
                            },
                            onFavoriteContactsChange = { value ->
                                scope.launch { configRepository.set("favorite_contacts", value) }
                            },
                            onBlockedKeywordsChange = { value ->
                                scope.launch { configRepository.set("blocked_keywords", value) }
                            },
                            appLockEnabled = appLockEnabled,
                            recoveryQuestion = recoveryQuestion.orEmpty(),
                            onAppLockSave = { enabled, secret, question, answer, onResult ->
                                scope.launch {
                                    if (!enabled) {
                                        configRepository.set("mobile_lock_enabled", "false")
                                        onResult("Mobile app lock disabled.")
                                        return@launch
                                    }
                                    if (!MobileAppLockCrypto.validSecret(secret)) {
                                        onResult("Use a 4+ digit PIN or 8+ character password.")
                                        return@launch
                                    }
                                    if (question.isBlank() || answer.isBlank()) {
                                        onResult("Choose a security question and answer.")
                                        return@launch
                                    }
                                    val secretSalt = MobileAppLockCrypto.newSalt()
                                    val answerSalt = MobileAppLockCrypto.newSalt()
                                    configRepository.set("mobile_lock_salt", secretSalt)
                                    configRepository.set("mobile_lock_hash", MobileAppLockCrypto.hashSecret(secret, secretSalt))
                                    configRepository.set("mobile_lock_recovery_question", question.trim())
                                    configRepository.set("mobile_lock_recovery_salt", answerSalt)
                                    configRepository.set("mobile_lock_recovery_hash", MobileAppLockCrypto.hashSecret(answer.lowercase(), answerSalt))
                                    configRepository.set("mobile_lock_enabled", "true")
                                    onResult("Mobile app lock enabled.")
                                }
                            },
                        )
                        AppTab.Log -> NotificationLog(
                            items = items,
                            clearOlderThan = { cutoffMs -> notifications.clearOlderThan(cutoffMs) },
                            clearBetween = { startMs, endMs -> notifications.clearBetween(startMs, endMs) },
                            deleteOne = { id -> notifications.delete(id) },
                            clearAll = { notifications.clearAll() },
                        )
                    }
                }

                NavigationBar(
                    modifier = Modifier
                        .clip(RoundedCornerShape(28.dp))
                        .border(1.dp, Color(0x1F17221E), RoundedCornerShape(28.dp)),
                    containerColor = Color(0xCCFFFAF0),
                ) {
                    AppTab.entries.forEach { item ->
                        NavigationBarItem(
                            selected = tab == item,
                            onClick = { tab = item },
                            icon = { Icon(appTabIcon(item), contentDescription = item.label) },
                            label = { if (!compact) Text(item.label) },
                        )
                    }
                }
            }
        }
    }
}

private fun appTabIcon(tab: AppTab) = when (tab) {
    AppTab.Home -> Icons.Filled.Home
    AppTab.Pair -> Icons.Filled.QrCodeScanner
    AppTab.Rules -> Icons.Filled.Tune
    AppTab.Log -> Icons.AutoMirrored.Filled.Article
}

private fun isNotificationAccessEnabled(context: android.content.Context): Boolean =
    Settings.Secure.getString(context.contentResolver, "enabled_notification_listeners")
        ?.contains(context.packageName, ignoreCase = true) == true

private fun isPostNotificationsAllowed(context: android.content.Context): Boolean =
    Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
        ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) ==
        PackageManager.PERMISSION_GRANTED

private fun isIgnoringBatteryOptimizations(context: android.content.Context): Boolean {
    val manager = context.getSystemService(PowerManager::class.java) ?: return false
    return manager.isIgnoringBatteryOptimizations(context.packageName)
}

@Composable
private fun Header(
    connectionState: ConnectionState,
    studyMode: Boolean,
    compact: Boolean,
    onStudyModeChange: (Boolean) -> Unit,
) {
    Card(
        shape = RoundedCornerShape(30.dp),
        colors = CardDefaults.cardColors(containerColor = Color(0xCCFFFAF0)),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
    ) {
        Column(Modifier.padding(if (compact) 14.dp else 20.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f)) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        Image(
                            painter = painterResource(R.drawable.app_logo),
                            contentDescription = "FocusBridge logo",
                            modifier = Modifier
                                .size(if (compact) 38.dp else 46.dp)
                                .clip(RoundedCornerShape(14.dp)),
                        )
                        Text(
                            "FocusBridge",
                            modifier = Modifier.weight(1f, fill = false),
                            style = if (compact) MaterialTheme.typography.titleLarge else MaterialTheme.typography.headlineMedium,
                            fontWeight = FontWeight.Black,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    Text(
                        "See everything. Open nothing.",
                        color = Color(0xFF61706A),
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
                Spacer(Modifier.width(8.dp))
                ConnectionBlinker(connectionState)
            }
            Spacer(Modifier.height(if (compact) 10.dp else 16.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column {
                    Text("Study Mode", fontWeight = FontWeight.Bold)
                    Text("Filter noise while you work", color = Color(0xFF61706A))
                }
                Switch(checked = studyMode, onCheckedChange = onStudyModeChange)
            }
        }
    }
}

@Composable
private fun ConnectionBlinker(state: ConnectionState) {
    val color = when (state) {
        ConnectionState.CONNECTED -> Color(0xFF24B86F)
        ConnectionState.CONNECTING -> Color(0xFFD6A840)
        ConnectionState.RETRYING -> Color(0xFFD6A840)
        ConnectionState.DISCONNECTED -> Color(0xFFE24D4D)
    }
    val label = when (state) {
        ConnectionState.CONNECTED -> "Connected"
        ConnectionState.CONNECTING -> "Connecting"
        ConnectionState.RETRYING -> "Retrying"
        ConnectionState.DISCONNECTED -> "Disconnected"
    }
    Row(
        modifier = Modifier
            .clip(RoundedCornerShape(999.dp))
            .background(Color(0x99FFFFFF))
            .border(1.dp, Color(0x1F17221E), RoundedCornerShape(999.dp))
            .padding(horizontal = 12.dp, vertical = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Box(
            modifier = Modifier
                .size(11.dp)
                .clip(CircleShape)
                .background(color),
        )
        Text(label, fontWeight = FontWeight.Bold, color = Color(0xFF17221E))
    }
}

@Composable
private fun HomeTab(
    items: List<NotificationEntity>,
    activePairing: PairingEntity?,
    connectionState: ConnectionState,
    notificationAccessEnabled: Boolean,
    postNotificationsAllowed: Boolean,
    batteryUnrestricted: Boolean,
    compact: Boolean,
    openNotificationAccess: () -> Unit,
    openBatterySettings: () -> Unit,
    startSync: () -> Unit,
    manualDisconnect: () -> Unit,
) {
    LazyColumn(verticalArrangement = Arrangement.spacedBy(14.dp)) {
        item {
            if (compact) {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    StatCard("Captured", items.size.toString())
                    StatCard("Priority", items.count { it.priority == "URGENT" || it.priority == "HIGH" }.toString())
                }
            } else {
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    StatCard("Captured", items.size.toString(), Modifier.weight(1f))
                    StatCard("Priority", items.count { it.priority == "URGENT" || it.priority == "HIGH" }.toString(), Modifier.weight(1f))
                }
            }
        }
        item {
            SetupChecklist(
                notificationAccessEnabled = notificationAccessEnabled,
                postNotificationsAllowed = postNotificationsAllowed,
                batteryUnrestricted = batteryUnrestricted,
                paired = activePairing != null,
                connected = connectionState == ConnectionState.CONNECTED,
                openNotificationAccess = openNotificationAccess,
                openBatterySettings = openBatterySettings,
                startSync = startSync,
            )
        }
        item {
            if (!notificationAccessEnabled) {
                ActionCard(
                    title = "Turn on notification access",
                    body = "Android requires one settings approval before FocusBridge can capture phone notifications.",
                    primary = "Open settings",
                    secondary = "Start sync",
                    onPrimary = openNotificationAccess,
                    onSecondary = startSync,
                )
            }
        }
        item {
            ActionCard(
                title = when {
                    activePairing == null -> "Pair your desktop"
                    connectionState == ConnectionState.CONNECTED -> "Desktop connected"
                    else -> "Desktop saved, reconnect needed"
                },
                body = activePairing?.endpoint ?: "Open desktop FocusBridge, show Pairing, then scan the QR code.",
                primary = if (connectionState == ConnectionState.CONNECTED) "Connected" else "Retry sync",
                secondary = if (connectionState == ConnectionState.CONNECTED) "Disconnect" else "Notification access",
                onPrimary = startSync,
                onSecondary = if (connectionState == ConnectionState.CONNECTED) manualDisconnect else openNotificationAccess,
                secondaryDestructive = connectionState == ConnectionState.CONNECTED,
            )
        }
        item {
            Text("Recent notifications", fontWeight = FontWeight.Black, color = Color(0xFF17221E))
        }
        items(items.take(6), key = { it.id }) { notification ->
            MobileNotificationRow(notification)
        }
    }
}

@Composable
private fun SetupChecklist(
    notificationAccessEnabled: Boolean,
    postNotificationsAllowed: Boolean,
    batteryUnrestricted: Boolean,
    paired: Boolean,
    connected: Boolean,
    openNotificationAccess: () -> Unit,
    openBatterySettings: () -> Unit,
    startSync: () -> Unit,
) {
    PanelCard {
        Text("Production setup", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Black)
        Text(
            "Complete these once for reliable LAN/hotspot sync.",
            color = Color(0xFF61706A),
        )
        ChecklistRow(
            label = "Notification access",
            body = "Required to capture phone notifications.",
            done = notificationAccessEnabled,
            action = "Open",
            onAction = openNotificationAccess,
        )
        ChecklistRow(
            label = "Android notification permission",
            body = "Needed for the persistent sync service indicator.",
            done = postNotificationsAllowed,
            action = "Check",
            onAction = startSync,
        )
        ChecklistRow(
            label = "Battery unrestricted",
            body = "Recommended so Android does not stop sync after a few minutes.",
            done = batteryUnrestricted,
            action = "Open",
            onAction = openBatterySettings,
        )
        ChecklistRow(
            label = "Desktop paired",
            body = "Scan the desktop QR or paste the manual payload.",
            done = paired,
            action = "Sync",
            onAction = startSync,
        )
        ChecklistRow(
            label = "Live connection",
            body = "Green means desktop accepted the pairing and heartbeat can recover disconnects.",
            done = connected,
            action = "Retry",
            onAction = startSync,
        )
    }
}

@Composable
private fun ChecklistRow(
    label: String,
    body: String,
    done: Boolean,
    action: String,
    onAction: () -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            if (done) Icons.Filled.CheckCircle else Icons.Filled.Warning,
            contentDescription = null,
            tint = if (done) Color(0xFF24B86F) else Color(0xFFD6A840),
            modifier = Modifier.size(24.dp),
        )
        Column(Modifier.weight(1f)) {
            Text(label, fontWeight = FontWeight.Bold, color = Color(0xFF17221E))
            Text(body, color = Color(0xFF61706A), style = MaterialTheme.typography.bodySmall)
        }
        Button(
            enabled = !done,
            onClick = onAction,
            colors = ButtonDefaults.buttonColors(
                containerColor = Color(0xFF17221E),
                disabledContainerColor = Color(0xFFE1ECE3),
                disabledContentColor = Color(0xFF61706A),
            ),
        ) {
            Text(if (done) "Done" else action)
        }
    }
}

@Composable
private fun PairTab(
    pairingManager: PairingManager,
    startSync: () -> Unit,
    compact: Boolean,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var qrPayload by remember { mutableStateOf("") }
    var pairingMessage by remember { mutableStateOf<String?>(null) }
    var scannerOpen by remember { mutableStateOf(false) }
    val cameraPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted ->
        scannerOpen = granted
        if (!granted) pairingMessage = "Camera permission is needed to scan the desktop QR."
    }

    LazyColumn(verticalArrangement = Arrangement.spacedBy(14.dp)) {
        item {
            PanelCard {
            Text("Pair with desktop", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Black)
            Text(
                "Scan the desktop QR code. Manual paste is still available for debugging.",
                color = Color(0xFF61706A),
            )
            Button(
                modifier = Modifier.fillMaxWidth(),
                onClick = {
                    if (ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) ==
                        PackageManager.PERMISSION_GRANTED
                    ) {
                        scannerOpen = true
                    } else {
                        cameraPermission.launch(Manifest.permission.CAMERA)
                    }
                },
                colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF17221E)),
            ) {
                Icon(Icons.Filled.CameraAlt, contentDescription = null)
                Spacer(Modifier.width(8.dp))
                Text("Scan QR with camera")
            }
            if (scannerOpen) {
                QrScanner(
                    compact = compact,
                    onPayload = { payload ->
                        scannerOpen = false
                        qrPayload = payload
                        scope.launch {
                            pairingMessage = savePairing(pairingManager, payload, startSync)
                        }
                    },
                    onClose = { scannerOpen = false },
                )
            }
            OutlinedTextField(
                value = qrPayload,
                onValueChange = { qrPayload = it },
                modifier = Modifier.fillMaxWidth(),
                minLines = if (compact) 3 else 5,
                label = { Text("Desktop QR payload") },
            )
            Button(
                modifier = Modifier.fillMaxWidth(),
                onClick = {
                    scope.launch {
                        pairingMessage = savePairing(pairingManager, qrPayload, startSync)
                    }
                },
                colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF17221E)),
            ) {
                Icon(Icons.Filled.Devices, contentDescription = null)
                Spacer(Modifier.width(8.dp))
                Text("Save pairing")
            }
            pairingMessage?.let { Text(it, color = Color(0xFF61706A)) }
            }
        }
    }
}

private suspend fun savePairing(
    pairingManager: PairingManager,
    payload: String,
    startSync: () -> Unit,
): String = runCatching { pairingManager.consume(payload) }
    .fold(
        onSuccess = {
            startSync()
            "Paired ${it.endpoint}. Starting sync..."
        },
        onFailure = { "Pairing failed: ${it.message}" },
    )

@Composable
private fun QrScanner(
    compact: Boolean,
    onPayload: (String) -> Unit,
    onClose: () -> Unit,
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val cameraExecutor = remember { Executors.newSingleThreadExecutor() }
    val mainExecutor = remember(context) { ContextCompat.getMainExecutor(context) }
    val completed = remember { AtomicBoolean(false) }

    DisposableEffect(Unit) {
        onDispose { cameraExecutor.shutdown() }
    }

    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(if (compact) 220.dp else 300.dp)
                .clip(RoundedCornerShape(24.dp))
                .border(1.dp, Color(0x3317221E), RoundedCornerShape(24.dp))
                .background(Color.Black),
        ) {
            AndroidView(
                modifier = Modifier.fillMaxSize(),
                factory = { ctx ->
                    val previewView = PreviewView(ctx).apply {
                        scaleType = PreviewView.ScaleType.FILL_CENTER
                    }
                    val cameraProviderFuture = ProcessCameraProvider.getInstance(ctx)
                    cameraProviderFuture.addListener(
                        {
                            val cameraProvider = cameraProviderFuture.get()
                            val preview = Preview.Builder().build().also {
                                it.setSurfaceProvider(previewView.surfaceProvider)
                            }
                            val analyzer = ImageAnalysis.Builder()
                                .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                                .build()
                                .also { imageAnalysis ->
                                    imageAnalysis.setAnalyzer(cameraExecutor) { imageProxy ->
                                        if (!completed.get()) {
                                            decodeQr(imageProxy)?.let { payload ->
                                                mainExecutor.execute {
                                                    if (completed.compareAndSet(false, true)) {
                                                        onPayload(payload)
                                                    }
                                                }
                                            }
                                        }
                                        imageProxy.close()
                                    }
                                }
                            runCatching {
                                cameraProvider.unbindAll()
                                cameraProvider.bindToLifecycle(
                                    lifecycleOwner,
                                    CameraSelector.DEFAULT_BACK_CAMERA,
                                    preview,
                                    analyzer,
                                )
                            }
                        },
                        ContextCompat.getMainExecutor(ctx),
                    )
                    previewView
                },
            )
            Text(
                "Align the desktop QR inside this frame",
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .fillMaxWidth()
                    .background(Color(0x99000000))
                    .padding(10.dp),
                color = Color.White,
                fontWeight = FontWeight.Bold,
            )
        }
        Text(
            "Camera frames stay on-device. Only the decoded pairing payload is saved locally.",
            color = Color(0xFF61706A),
            style = MaterialTheme.typography.bodySmall,
        )
        Text(
            "Close scanner",
            modifier = Modifier
                .clip(RoundedCornerShape(999.dp))
                .clickable(onClick = onClose)
                .background(Color(0xFFECE3D1))
                .padding(horizontal = 14.dp, vertical = 9.dp),
            color = Color(0xFF17221E),
            fontWeight = FontWeight.Bold,
        )
    }
}

private fun decodeQr(imageProxy: ImageProxy): String? {
    val width = imageProxy.width
    val height = imageProxy.height
    val plane = imageProxy.planes.firstOrNull() ?: return null
    val buffer = plane.buffer
    val rowStride = plane.rowStride
    val yData = ByteArray(width * height)

    for (row in 0 until height) {
        val rowStart = row * rowStride
        if (rowStart + width > buffer.capacity()) return null
        buffer.position(rowStart)
        buffer.get(yData, row * width, width)
    }

    return runCatching {
        val source = PlanarYUVLuminanceSource(
            yData,
            width,
            height,
            0,
            0,
            width,
            height,
            false,
        )
        val bitmap = BinaryBitmap(HybridBinarizer(source))
        val reader = MultiFormatReader().apply {
            setHints(mapOf(DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE)))
        }
        reader.decode(bitmap).text
    }.getOrNull()
}

@Composable
private fun MobileAppLockGate(
    appLockSalt: String?,
    appLockHash: String?,
    recoveryQuestion: String?,
    recoveryHash: String?,
    onUnlock: () -> Unit,
    onResetSecret: (String, String, (String) -> Unit) -> Unit,
) {
    var secret by remember { mutableStateOf("") }
    var answer by remember { mutableStateOf("") }
    var newSecret by remember { mutableStateOf("") }
    var resetOpen by remember { mutableStateOf(false) }
    var message by remember { mutableStateOf<String?>(null) }

    Surface(modifier = Modifier.fillMaxSize(), color = Color(0xFFF5EFE4)) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(Brush.verticalGradient(listOf(Color(0xFFF9F2E7), Color(0xFFE1ECE3))))
                .padding(18.dp),
            contentAlignment = Alignment.Center,
        ) {
            PanelCard(Modifier.fillMaxWidth()) {
                Icon(Icons.Filled.Security, contentDescription = null, tint = Color(0xFF3F7F70), modifier = Modifier.size(42.dp))
                Text("Unlock FocusBridge", style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Black)
                Text("Enter your local PIN or password. This stays on this phone.", color = Color(0xFF61706A))
                OutlinedTextField(
                    value = secret,
                    onValueChange = { secret = it },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    label = { Text("PIN or password") },
                )
                Button(
                    modifier = Modifier.fillMaxWidth(),
                    onClick = {
                        if (MobileAppLockCrypto.verify(secret, appLockSalt, appLockHash)) {
                            secret = ""
                            onUnlock()
                        } else {
                            message = "Incorrect PIN/password."
                        }
                    },
                    colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF17221E)),
                ) {
                    Text("Unlock")
                }
                TextButton(onClick = { resetOpen = !resetOpen }) {
                    Text("Forgot PIN/password?")
                }
                if (resetOpen) {
                    Text(recoveryQuestion ?: "Security question is not configured.", fontWeight = FontWeight.Bold, color = Color(0xFF17221E))
                    OutlinedTextField(
                        value = answer,
                        onValueChange = { answer = it },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                        label = { Text("Security answer") },
                    )
                    OutlinedTextField(
                        value = newSecret,
                        onValueChange = { newSecret = it },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                        label = { Text("New PIN or password") },
                    )
                    Button(
                        modifier = Modifier.fillMaxWidth(),
                        enabled = !recoveryQuestion.isNullOrBlank() && !recoveryHash.isNullOrBlank(),
                        onClick = { onResetSecret(newSecret, answer) { result -> message = result } },
                        colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF3F7F70)),
                    ) {
                        Text("Reset and unlock")
                    }
                }
                message?.let { Text(it, color = Color(0xFF8F3324)) }
            }
        }
    }
}

@Composable
private fun RulesTab(
    studyMode: Boolean,
    privacyMode: Boolean,
    priorityKeywords: String,
    favoriteContacts: String,
    blockedKeywords: String,
    onStudyModeChange: (Boolean) -> Unit,
    onPrivacyModeChange: (Boolean) -> Unit,
    onPriorityKeywordsChange: (String) -> Unit,
    onFavoriteContactsChange: (String) -> Unit,
    onBlockedKeywordsChange: (String) -> Unit,
    appLockEnabled: Boolean,
    recoveryQuestion: String,
    onAppLockSave: (Boolean, String, String, String, (String) -> Unit) -> Unit,
) {
    var lockEnabled by remember(appLockEnabled) { mutableStateOf(appLockEnabled) }
    var lockSecret by remember { mutableStateOf("") }
    var lockQuestion by remember(recoveryQuestion) {
        mutableStateOf(if (recoveryQuestion.isBlank() || recoveryQuestion in MOBILE_SECURITY_QUESTIONS) recoveryQuestion else "Custom question")
    }
    var customLockQuestion by remember(recoveryQuestion) {
        mutableStateOf(if (recoveryQuestion !in MOBILE_SECURITY_QUESTIONS) recoveryQuestion else "")
    }
    var lockAnswer by remember { mutableStateOf("") }
    var lockMessage by remember { mutableStateOf<String?>(null) }

    LazyColumn(verticalArrangement = Arrangement.spacedBy(14.dp)) {
        item {
            RuleCard(
                "Masked Peek Mode",
                "Hide message text on desktop until you hover, focus, or tap the notification.",
                privacyMode,
                onPrivacyModeChange,
            )
        }
        item {
            KeywordRuleCard(
                title = "Priority keywords",
                body = "Words like OTP, deadline, invoice, or school can jump to high priority.",
                value = priorityKeywords,
                placeholder = "otp, deadline, payment",
                onValueChange = onPriorityKeywordsChange,
            )
        }
        item {
            KeywordRuleCard(
                title = "Favorite contacts",
                body = "Names here always stay visible as high priority, even during Study Mode.",
                value = favoriteContacts,
                placeholder = "Mom, boss, project lead",
                onValueChange = onFavoriteContactsChange,
            )
        }
        item {
            KeywordRuleCard(
                title = "Blocked keywords",
                body = "Mute noisy words before they ever reach the desktop.",
                value = blockedKeywords,
                placeholder = "promo, sale, newsletter",
                onValueChange = onBlockedKeywordsChange,
            )
        }
        item {
            RuleCard("Study Mode", "Suppress low-priority noise while keeping urgent alerts visible.", studyMode, onStudyModeChange)
        }
        item {
            RuleCard("2FA fast lane", "Security codes and sign-in prompts stay easy to find.", true) {}
        }
        item {
            RuleCard("Local-first sync", "Notification data stays on your devices for the MVP.", true) {}
        }
        item {
            PanelCard {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text("Mobile app lock", fontWeight = FontWeight.Black, color = Color(0xFF17221E))
                        Text("Optional local PIN/password gate for this phone.", color = Color(0xFF61706A))
                    }
                    Switch(checked = lockEnabled, onCheckedChange = { lockEnabled = it })
                }
                if (lockEnabled) {
                    OutlinedTextField(
                        value = lockSecret,
                        onValueChange = { lockSecret = it },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                        label = { Text("PIN or password") },
                    )
                    Text("Use a 4+ digit PIN or 8+ character password.", color = Color(0xFF61706A), style = MaterialTheme.typography.bodySmall)
                    Text("Security question", fontWeight = FontWeight.Bold, color = Color(0xFF17221E))
                    MOBILE_SECURITY_QUESTIONS.forEach { question ->
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(18.dp))
                                .clickable { lockQuestion = question }
                                .background(if (lockQuestion == question) Color(0x223F7F70) else Color.Transparent)
                                .padding(10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Icon(
                                if (lockQuestion == question) Icons.Filled.CheckCircle else Icons.Filled.Security,
                                contentDescription = null,
                                tint = if (lockQuestion == question) Color(0xFF24B86F) else Color(0xFF61706A),
                                modifier = Modifier.size(18.dp),
                            )
                            Spacer(Modifier.width(8.dp))
                            Text(question, color = Color(0xFF17221E))
                        }
                    }
                    if (lockQuestion == "Custom question") {
                        OutlinedTextField(
                            value = customLockQuestion,
                            onValueChange = { customLockQuestion = it },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                            label = { Text("Custom security question") },
                        )
                    }
                    OutlinedTextField(
                        value = lockAnswer,
                        onValueChange = { lockAnswer = it },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                        label = { Text("Security answer") },
                    )
                }
                Button(
                    modifier = Modifier.fillMaxWidth(),
                    onClick = {
                        val selectedQuestion = if (lockQuestion == "Custom question") customLockQuestion else lockQuestion
                        onAppLockSave(lockEnabled, lockSecret, selectedQuestion, lockAnswer) { result -> lockMessage = result }
                    },
                    colors = ButtonDefaults.buttonColors(containerColor = if (lockEnabled) Color(0xFF17221E) else Color(0xFF8F3324)),
                ) {
                    Text(if (lockEnabled) "Save app lock" else "Turn off app lock")
                }
                lockMessage?.let { Text(it, color = Color(0xFF61706A)) }
            }
        }
    }
}

@Composable
private fun NotificationLog(
    items: List<NotificationEntity>,
    clearOlderThan: suspend (Long) -> Int,
    clearBetween: suspend (Long, Long) -> Int,
    deleteOne: suspend (String) -> Int,
    clearAll: suspend () -> Int,
) {
    val scope = rememberCoroutineScope()
    var customDays by remember { mutableStateOf("14") }
    var clearMessage by remember { mutableStateOf<String?>(null) }

    fun clear(days: Int) {
        val cutoffMs = System.currentTimeMillis() - days * 24L * 60L * 60L * 1000L
        scope.launch {
            val deleted = clearOlderThan(cutoffMs)
            clearMessage = "Cleared $deleted notifications older than $days day${if (days == 1) "" else "s"}."
        }
    }

    fun clearEverything() {
        scope.launch {
            val deleted = clearAll()
            clearMessage = "Cleared $deleted captured notifications."
        }
    }

    LazyColumn(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        item {
            PanelCard {
                Text("Clear history", fontWeight = FontWeight.Black)
                Text(
                    "Delete old captured notifications from this phone. Your original apps are untouched.",
                    color = Color(0xFF61706A),
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf(1 to "1 day", 7 to "7 days", 30 to "1 month").forEach { (days, label) ->
                        Button(
                            modifier = Modifier.weight(1f),
                            onClick = { clear(days) },
                            colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF3F7F70)),
                        ) {
                            Text(label, maxLines = 1)
                        }
                    }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    OutlinedTextField(
                        value = customDays,
                        onValueChange = { customDays = it.filter(Char::isDigit).take(4) },
                        modifier = Modifier.weight(1f),
                        singleLine = true,
                        label = { Text("Custom days") },
                    )
                    Button(
                        onClick = {
                            customDays.toIntOrNull()?.takeIf { it > 0 }?.let(::clear)
                        },
                        colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF17221E)),
                    ) {
                        Text("Clear")
                    }
                }
                Button(
                    modifier = Modifier.fillMaxWidth(),
                    onClick = ::clearEverything,
                    colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF8F3324)),
                ) {
                    Text("Clear all captured messages")
                }
                clearMessage?.let { Text(it, color = Color(0xFF61706A)) }
            }
        }
        notificationSections(items).forEach { section ->
            item(section.label) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(section.label, fontWeight = FontWeight.Black, color = Color(0xFF17221E))
                    Button(
                        onClick = {
                            scope.launch {
                                val deleted = clearBetween(section.startMs, section.endMs)
                                clearMessage = "Deleted $deleted ${section.label.lowercase()} notifications."
                            }
                        },
                        colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF8F3324)),
                    ) {
                        Text("Delete section")
                    }
                }
            }
            items(section.items, key = { it.id }) { notification ->
                MobileNotificationRow(
                    notification = notification,
                    onDelete = {
                        scope.launch {
                            deleteOne(notification.id)
                        }
                    },
                )
            }
        }
    }
}

private data class MobileNotificationSection(
    val label: String,
    val startMs: Long,
    val endMs: Long,
    val items: List<NotificationEntity>,
)

private fun notificationSections(items: List<NotificationEntity>): List<MobileNotificationSection> {
    val now = System.currentTimeMillis()
    val hour = 60L * 60L * 1000L
    val day = 24L * hour
    return listOf(
        MobileNotificationSection("Just now", now - 5L * 60L * 1000L, Long.MAX_VALUE, emptyList()),
        MobileNotificationSection("Last hour", now - hour, now - 5L * 60L * 1000L, emptyList()),
        MobileNotificationSection("Today", now - day, now - hour, emptyList()),
        MobileNotificationSection("This week", now - 7L * day, now - day, emptyList()),
        MobileNotificationSection("This month", now - 30L * day, now - 7L * day, emptyList()),
        MobileNotificationSection("Older", 0L, now - 30L * day, emptyList()),
    ).map { section ->
        section.copy(
            items = items.filter { item ->
                val at = minOf(item.timestamp, item.receivedAt)
                at >= section.startMs && at < section.endMs
            },
        )
    }.filter { it.items.isNotEmpty() }
}

@Composable
private fun StatCard(label: String, value: String, modifier: Modifier = Modifier) {
    PanelCard(modifier) {
        Text(label, color = Color(0xFF61706A), style = MaterialTheme.typography.labelLarge)
        Text(value, style = MaterialTheme.typography.displaySmall, fontWeight = FontWeight.Black)
    }
}

@Composable
private fun ActionCard(
    title: String,
    body: String,
    primary: String,
    secondary: String,
    onPrimary: () -> Unit,
    onSecondary: () -> Unit,
    secondaryDestructive: Boolean = false,
) {
    PanelCard {
        Text(title, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Black)
        Text(body, color = Color(0xFF61706A))
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Button(
                modifier = Modifier.fillMaxWidth(),
                onClick = onPrimary,
                colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF17221E)),
            ) {
                Icon(if (primary.contains("settings", ignoreCase = true)) Icons.Filled.Settings else Icons.Filled.Sync, contentDescription = null)
                Spacer(Modifier.width(8.dp))
                Text(primary)
            }
            Button(
                modifier = Modifier.fillMaxWidth(),
                onClick = onSecondary,
                colors = ButtonDefaults.buttonColors(
                    containerColor = if (secondaryDestructive) Color(0xFF8F3324) else Color(0xFF3F7F70),
                ),
            ) {
                Icon(
                    if (secondaryDestructive) Icons.Filled.Block else Icons.Filled.NotificationsActive,
                    contentDescription = null,
                )
                Spacer(Modifier.width(8.dp))
                Text(secondary)
            }
        }
    }
}

@Composable
private fun RuleCard(
    title: String,
    body: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    PanelCard {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Icon(ruleIcon(title), contentDescription = null, tint = Color(0xFF3F7F70), modifier = Modifier.size(20.dp))
                    Text(title, fontWeight = FontWeight.Black)
                }
                Text(body, color = Color(0xFF61706A))
            }
            Switch(checked = checked, onCheckedChange = onCheckedChange)
        }
    }
}

@Composable
private fun KeywordRuleCard(
    title: String,
    body: String,
    value: String,
    placeholder: String,
    onValueChange: (String) -> Unit,
) {
    PanelCard {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Icon(ruleIcon(title), contentDescription = null, tint = Color(0xFF3F7F70), modifier = Modifier.size(20.dp))
            Text(title, fontWeight = FontWeight.Black)
        }
        Text(body, color = Color(0xFF61706A))
        OutlinedTextField(
            value = value,
            onValueChange = onValueChange,
            modifier = Modifier.fillMaxWidth(),
            minLines = 2,
            label = { Text(placeholder) },
        )
        Text("Separate entries with commas or new lines.", color = Color(0xFF9A8F7C), style = MaterialTheme.typography.labelSmall)
    }
}

private fun ruleIcon(title: String) = when {
    title.contains("Masked", ignoreCase = true) -> Icons.Filled.Shield
    title.contains("Priority", ignoreCase = true) -> Icons.Filled.Star
    title.contains("Favorite", ignoreCase = true) -> Icons.Filled.Star
    title.contains("Blocked", ignoreCase = true) -> Icons.Filled.Block
    title.contains("2FA", ignoreCase = true) -> Icons.Filled.Security
    title.contains("Local", ignoreCase = true) -> Icons.Filled.Devices
    else -> Icons.Filled.Tune
}

@Composable
private fun MobileNotificationRow(notification: NotificationEntity, onDelete: (() -> Unit)? = null) {
    PanelCard {
        Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Box(
                modifier = Modifier
                    .size(42.dp)
                    .clip(RoundedCornerShape(16.dp))
                    .background(Color(0xFFECE3D1)),
                contentAlignment = Alignment.Center,
            ) {
                Icon(Icons.Filled.NotificationsActive, contentDescription = null, tint = Color(0xFF17221E))
            }
            Column(Modifier.weight(1f)) {
                Text(
                    "${notification.appName} ${notification.sender.orEmpty()}",
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    fontWeight = FontWeight.Bold,
                )
                Text(
                    notification.message ?: "New notification",
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    color = Color(0xFF61706A),
                )
                Text(notification.priority, color = Color(0xFF9A8F7C), style = MaterialTheme.typography.labelSmall)
            }
            if (onDelete != null) {
                IconButton(onClick = onDelete) {
                    Icon(Icons.Filled.Delete, contentDescription = "Delete notification", tint = Color(0xFF8F3324))
                }
            }
        }
    }
}

@Composable
private fun PanelCard(
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    Card(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(28.dp),
        colors = CardDefaults.cardColors(containerColor = Color(0xCCFFFAF0)),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
    ) {
        Column(
            modifier = Modifier.padding(18.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            content = content,
        )
    }
}

@Composable
private fun FocusBridgeTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        typography = MaterialTheme.typography.copy(
            headlineMedium = MaterialTheme.typography.headlineMedium.copy(fontFamily = FontFamily.SansSerif),
        ),
        content = content,
    )
}
