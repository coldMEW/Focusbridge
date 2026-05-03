package com.focusbridge.android.priority

import javax.inject.Inject

enum class StudyModeDecision {
    SHOW_NOW,
    QUEUE_FOR_BATCH,
}

class StudyModeManager @Inject constructor() {
    fun decide(studyModeEnabled: Boolean, priority: Priority): StudyModeDecision {
        if (!studyModeEnabled) return StudyModeDecision.SHOW_NOW
        return when (priority) {
            Priority.URGENT,
            Priority.HIGH -> StudyModeDecision.SHOW_NOW
            Priority.NORMAL,
            Priority.LOW -> StudyModeDecision.QUEUE_FOR_BATCH
        }
    }
}
