export interface AppSettings {
  window: {
    x:number
    y:number
    pos_x: number
    pos_y: number
  }

  last_seen_version?: string

  interface: {
    theme?: "dark" | "light" | "system"
    language: string
    custom_locales_dir?: string
    path_log: string
  }

  overlay: {
    x:number
    y:number
    show: boolean
    pos_x: number
    pos_y: number
    overlay_browser: boolean
    overlay_browser_addr: string
    overlay_transparent: number
    run_name: boolean
    show_splits: boolean
    number_of_splits: number
    time_accuracy?: "seconds" | "tenths" | "hundredths" | "milliseconds"
    time_gold?: "segments" | "splits"
    split_separators: boolean
    group_list: boolean
    sum_of_best: boolean
    predicting_timer: boolean
    fake_timer: boolean
    run_aborted: boolean
    sum_of_the_last: number
    toggle_visibility_key: string
    drag_mode: boolean
    toggle_mode_key: string
    run_reset_key: string
  }
}
