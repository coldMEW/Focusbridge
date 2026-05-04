package com.focusbridge.android.data.repository

import com.focusbridge.android.data.local.AppRuleDao
import com.focusbridge.android.data.local.AppRuleEntity
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class AppRuleRepository @Inject constructor(
    private val dao: AppRuleDao,
) {
    suspend fun get(packageName: String): AppRuleEntity? = dao.get(packageName)

    suspend fun replaceFromDesktop(rules: List<AppRuleEntity>) {
        dao.upsertAll(rules)
    }
}
