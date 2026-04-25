export interface AppSettings {
  window: {
    x:number
    y:number
    pos_x: number
    pos_y: number
  }

  interface: {
    theme?: "dark" | "light" | "system"
    language: string
    path_log: string
  }

  overlay: {
    show: boolean
    pos_x: number
    pos_y: number
    overlay_transparent: number
    run_name: boolean
    show_splits: boolean
    number_of_splits: number
    split_separators: boolean
    sum_of_best: boolean
    fake_timer: boolean
    run_aborted: boolean
    sum_of_the_last: number
    time_accuracy?: "seconds" | "tenths" | "hundredths" | "milliseconds"
    netracell_tip: boolean
    toggle_visibility_key: string
    drag_mode: boolean
    toggle_mode_key: string
    run_reset_key: string
  }
}
