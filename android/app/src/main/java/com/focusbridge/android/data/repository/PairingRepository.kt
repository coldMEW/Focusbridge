package com.focusbridge.android.data.repository

import com.focusbridge.android.data.local.PairingDao
import com.focusbridge.android.data.local.PairingEntity
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class PairingRepository @Inject constructor(
    private val dao: PairingDao,
) {
    fun observeActive(): Flow<PairingEntity?> = dao.observeActive()

    suspend fun active(): PairingEntity? = dao.active()

    suspend fun save(pairing: PairingEntity) {
        dao.deactivateAll()
        dao.upsert(pairing.copy(active = true))
    }
}
