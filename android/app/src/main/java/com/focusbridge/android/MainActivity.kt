package com.focusbridge.android

import android.content.Intent
import android.os.Bundle
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
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
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
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
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import com.focusbridge.android.data.local.NotificationEntity
import com.focusbridge.android.data.local.PairingEntity
import com.focusbridge.android.data.repository.ConfigRepository
import com.focusbridge.android.data.repository.NotificationRepository
import com.focusbridge.android.data.repository.PairingRepository
import com.focusbridge.android.pairing.PairingManager
import com.focusbridge.android.service.SyncForegroundService
import com.focusbridge.android.sync.ConnectionState
import com.focusbridge.android.sync.WebSocketClient
import dagger.hilt.android.AndroidEntryPoint
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
        setContent {
            FocusBridgeTheme {
                FocusBridgeScreen(
                    notifications = notifications,
                    pairingRepository = pairingRepository,
                    pairingManager = pairingManager,
                    configRepository = configRepository,
                    connectionState = webSocketClient.state.collectAsState().value,
                    openNotificationAccess = {
                        startActivity(Intent(Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS))
                    },
                    startSync = {
                        ContextCompat.startForegroundService(
                            this,
                            Intent(this, SyncForegroundService::class.java),
                        )
                    },
                )
            }
        }
    }
}

private enum class AppTab(val label: String) {
    Home("Home"),
    Pair("Pair"),
    Rules("Rules"),
    Log("Log"),
}

@Composable
private fun FocusBridgeScreen(
    notifications: NotificationRepository,
    pairingRepository: PairingRepository,
    pairingManager: PairingManager,
    configRepository: ConfigRepository,
    connectionState: ConnectionState,
    openNotificationAccess: () -> Unit,
    startSync: () -> Unit,
) {
    val items by notifications.observeRecent().collectAsState(initial = emptyList())
    val activePairing by pairingRepository.observeActive().collectAsState(initial = null)
    val studyModeFlow = remember(configRepository) {
        configRepository.observe("study_mode_enabled").map { it == "true" }
    }
    val studyMode by studyModeFlow.collectAsState(initial = false)
    var tab by remember { mutableStateOf(AppTab.Home) }
    val scope = rememberCoroutineScope()

    LaunchedEffect(Unit) {
        startSync()
    }

    Surface(
        modifier = Modifier.fillMaxSize(),
        color = Color(0xFFF5EFE4),
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .background(
                    Brush.verticalGradient(
                        colors = listOf(Color(0xFFF9F2E7), Color(0xFFE1ECE3)),
                    ),
                )
                .padding(18.dp),
        ) {
            Header(connectionState = connectionState, studyMode = studyMode) { on ->
                scope.launch { configRepository.set("study_mode_enabled", on.toString()) }
            }
            Spacer(Modifier.height(16.dp))

            Box(modifier = Modifier.weight(1f)) {
                when (tab) {
                    AppTab.Home -> HomeTab(
                        items = items,
                        activePairing = activePairing,
                        connectionState = connectionState,
                        openNotificationAccess = openNotificationAccess,
                        startSync = startSync,
                    )
                    AppTab.Pair -> PairTab(pairingManager = pairingManager)
                    AppTab.Rules -> RulesTab(
                        studyMode = studyMode,
                        onStudyModeChange = { on ->
                            scope.launch { configRepository.set("study_mode_enabled", on.toString()) }
                        },
                    )
                    AppTab.Log -> NotificationLog(items = items)
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
                        icon = { Text(item.label.take(1), fontWeight = FontWeight.Bold) },
                        label = { Text(item.label) },
                    )
                }
            }
        }
    }
}

@Composable
private fun Header(
    connectionState: ConnectionState,
    studyMode: Boolean,
    onStudyModeChange: (Boolean) -> Unit,
) {
    Card(
        shape = RoundedCornerShape(30.dp),
        colors = CardDefaults.cardColors(containerColor = Color(0xCCFFFAF0)),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
    ) {
        Column(Modifier.padding(20.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column {
                    Text(
                        "FocusBridge",
                        style = MaterialTheme.typography.headlineMedium,
                        fontWeight = FontWeight.Black,
                    )
                    Text(
                        "See everything. Open nothing.",
                        color = Color(0xFF61706A),
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
                ConnectionBlinker(connectionState)
            }
            Spacer(Modifier.height(16.dp))
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
    openNotificationAccess: () -> Unit,
    startSync: () -> Unit,
) {
    LazyColumn(verticalArrangement = Arrangement.spacedBy(14.dp)) {
        item {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                StatCard("Captured", items.size.toString(), Modifier.weight(1f))
                StatCard("Priority", items.count { it.priority == "URGENT" || it.priority == "HIGH" }.toString(), Modifier.weight(1f))
            }
        }
        item {
            ActionCard(
                title = if (activePairing == null) "Pair your desktop" else "Desktop ready",
                body = activePairing?.endpoint ?: "Open desktop FocusBridge and paste the QR payload in Pair.",
                primary = if (connectionState == ConnectionState.CONNECTED) "Connected" else "Start sync",
                secondary = "Notification access",
                onPrimary = startSync,
                onSecondary = openNotificationAccess,
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
private fun PairTab(pairingManager: PairingManager) {
    val scope = rememberCoroutineScope()
    var qrPayload by remember { mutableStateOf("") }
    var pairingMessage by remember { mutableStateOf<String?>(null) }

    Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
        PanelCard {
            Text("Pair with desktop", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Black)
            Text(
                "Scan support is next, but paste works now. The payload stores endpoint, key, and certificate fingerprint locally.",
                color = Color(0xFF61706A),
            )
            OutlinedTextField(
                value = qrPayload,
                onValueChange = { qrPayload = it },
                modifier = Modifier.fillMaxWidth(),
                minLines = 5,
                label = { Text("Desktop QR payload") },
            )
            Button(
                onClick = {
                    scope.launch {
                        pairingMessage = runCatching { pairingManager.consume(qrPayload) }
                            .fold(
                                onSuccess = { "Paired ${it.endpoint}" },
                                onFailure = { "Pairing failed: ${it.message}" },
                            )
                    }
                },
                colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF17221E)),
            ) {
                Text("Save pairing")
            }
            pairingMessage?.let { Text(it, color = Color(0xFF61706A)) }
        }
    }
}

@Composable
private fun RulesTab(
    studyMode: Boolean,
    onStudyModeChange: (Boolean) -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
        RuleCard("Study Mode", "Suppress low-priority noise while keeping urgent alerts visible.", studyMode, onStudyModeChange)
        RuleCard("2FA fast lane", "Security codes and sign-in prompts stay easy to find.", true) {}
        RuleCard("Local-first sync", "Notification data stays on your devices for the MVP.", true) {}
    }
}

@Composable
private fun NotificationLog(items: List<NotificationEntity>) {
    LazyColumn(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        items(items, key = { it.id }) { notification ->
            MobileNotificationRow(notification)
        }
    }
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
) {
    PanelCard {
        Text(title, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Black)
        Text(body, color = Color(0xFF61706A))
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Button(onClick = onPrimary, colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF17221E))) {
                Text(primary)
            }
            Button(onClick = onSecondary, colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF3F7F70))) {
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
                Text(title, fontWeight = FontWeight.Black)
                Text(body, color = Color(0xFF61706A))
            }
            Switch(checked = checked, onCheckedChange = onCheckedChange)
        }
    }
}

@Composable
private fun MobileNotificationRow(notification: NotificationEntity) {
    PanelCard {
        Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Box(
                modifier = Modifier
                    .size(42.dp)
                    .clip(RoundedCornerShape(16.dp))
                    .background(Color(0xFFECE3D1)),
                contentAlignment = Alignment.Center,
            ) {
                Text(notification.appName.take(2).uppercase(), fontWeight = FontWeight.Black)
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
