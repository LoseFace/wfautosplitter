import { listen } from "@tauri-apps/api/event";
import { ref } from "vue";

export interface OverlaySplit {
    id: string;
    name: string;
    group_id: string;
    order: number;
    is_completed: boolean;
    split_time: number | null;
}

export interface OverlayState {
    template_name: string;
    template_id: string;
    sequential_mode: boolean;
    splits: OverlaySplit[];
    current_timer: number | null;
    is_running: boolean;
    is_trigger_only: boolean;
}


export const timerStartTime = ref(0)

export const overlayState = ref<OverlayState>({
    template_name: "",
    template_id: "",
    sequential_mode: false,
    splits: [],
    current_timer: null,
    is_running: false,
    is_trigger_only: false,
});

export const firstSplitReceived = ref(false)
let startTimerCallback: (() => void) | null = null
let stopTimerCallback: ((finalTime: number | null) => void) | null = null
let pauseTimerCallback: ((frozenTime: number) => void) | null = null
let resumeTimerCallback: (() => void) | null = null

export function setStartTimerCallback(callback: () => void) {
    startTimerCallback = callback
}
export function setStopTimerCallback(callback: (finalTime: number | null) => void) {
    stopTimerCallback = callback
}
export function setPauseTimerCallback(callback: (frozenTime: number) => void) {
    pauseTimerCallback = callback
}
export function setResumeTimerCallback(callback: () => void) {
    resumeTimerCallback = callback
}

export function initOverlayListener() {
    listen<OverlayState>("overlay-update", (event) => {
        overlayState.value = event.payload
        console.log("Overlay updated:", overlayState.value.is_trigger_only, overlayState.value.is_running)
        
        if (overlayState.value.is_trigger_only && overlayState.value.is_running && startTimerCallback) {
            console.log("Trigger-only template detected, starting timer from overlay.ts")
            startTimerCallback()
        }
        
        if (!overlayState.value.is_running && overlayState.value.current_timer !== null && stopTimerCallback) {
            console.log("Run finished, stopping timer with final time:", overlayState.value.current_timer)
            stopTimerCallback(overlayState.value.current_timer)
        }
    })

    listen("first-split-received", () => {
        console.log("First split received, timer should start")
        if (startTimerCallback && overlayState.value.is_running) {
            startTimerCallback()
        }
        firstSplitReceived.value = true
    })
    
    listen<number>("timer-pause", (event) => {
        console.log("Timer pause received, frozen at:", event.payload)
        if (pauseTimerCallback) {
            pauseTimerCallback(event.payload)
        }
    })

    listen("timer-resume", () => {
        console.log("Timer resume received")
        if (resumeTimerCallback) {
            resumeTimerCallback()
        }
    })

    listen("run-reset", () => {
        console.log("Run reset, resetting timer flags")
        firstSplitReceived.value = false
        if (stopTimerCallback) {
            stopTimerCallback(null)
        }
    })
}