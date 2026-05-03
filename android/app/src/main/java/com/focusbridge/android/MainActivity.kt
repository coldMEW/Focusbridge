package com.focusbridge.android

import android.content.Intent
import android.os.Bundle
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import com.focusbridge.android.data.local.NotificationEntity
import com.focusbridge.android.data.repository.ConfigRepository
import com.focusbridge.android.data.repository.NotificationRepository
import com.focusbridge.android.pairing.PairingManager
import com.focusbridge.android.service.SyncForegroundService
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import javax.inject.Inject

@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    @Inject lateinit var notifications: NotificationRepository
    @Inject lateinit var pairingManager: PairingManager
    @Inject lateinit var configRepository: ConfigRepository

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme {
                Surface {
                    FocusBridgeScreen(
                        notifications = notifications,
                        pairingManager = pairingManager,
                        configRepository = configRepository,
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
}

@Composable
private fun FocusBridgeScreen(
    notifications: NotificationRepository,
    pairingManager: PairingManager,
    configRepository: ConfigRepository,
    openNotificationAccess: () -> Unit,
    startSync: () -> Unit,
) {
    val items by notifications.observeRecent().collectAsState(initial = emptyList())
    val studyModeFlow = remember(configRepository) {
        configRepository.observe("study_mode_enabled").map { it == "true" }
    }
    val studyMode by studyModeFlow.collectAsState(initial = false)
    val scope = rememberCoroutineScope()
    var qrPayload by remember { mutableStateOf("") }
    var pairingMessage by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(Unit) {
        startSync()
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Column {
                Text("FocusBridge", style = MaterialTheme.typography.titleLarge)
                Text("See everything. Open nothing.", style = MaterialTheme.typography.bodySmall)
            }
            Switch(
                checked = studyMode,
                onCheckedChange = { on ->
                    scope.launch { configRepository.set("study_mode_enabled", on.toString()) }
                },
            )
        }

        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Button(onClick = openNotificationAccess) {
                Text("Notification access")
            }
            Button(onClick = startSync) {
                Text("Start sync")
            }
        }

        OutlinedTextField(
            value = qrPayload,
            onValueChange = { qrPayload = it },
            modifier = Modifier.fillMaxWidth(),
            minLines = 3,
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
        ) {
            Text("Save pairing")
        }
        pairingMessage?.let { Text(it, style = MaterialTheme.typography.bodySmall) }

        HorizontalDivider()
        Text("Captured notifications", style = MaterialTheme.typography.titleMedium)
        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(items, key = { it.id }) { notification ->
                NotificationRow(notification)
            }
        }
    }
}

@Composable
private fun NotificationRow(notification: NotificationEntity) {
    Column {
        Text("[${notification.appName}] ${notification.sender.orEmpty()}")
        Text(notification.message ?: "New message", style = MaterialTheme.typography.bodySmall)
        Text(notification.priority, style = MaterialTheme.typography.labelSmall)
    }
}
