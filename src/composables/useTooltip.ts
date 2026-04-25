import { ref, reactive } from 'vue'

export function useTooltip() {
  
  const tooltipEl = ref<HTMLElement | null>(null)

  const tooltipState = reactive({
    visible: false,
    text: '',
    x: 0,
    y: 0
  })

  const showTooltip = (event: MouseEvent, text: string) => {
    tooltipState.text = text
    tooltipState.visible = true

    requestAnimationFrame(() => {
      if (tooltipState.visible) {
        updateTooltipPosition(event)
      }
    })
  }

  const moveTooltip = (event: MouseEvent) => {
    if (!tooltipState.visible) return
    updateTooltipPosition(event)
  }

  const hideTooltip = () => {
    tooltipState.visible = false
  }

  const updateTooltipPosition = (event: MouseEvent) => {
    const offset = 12
    let x = event.clientX + offset
    let y = event.clientY + offset

    const tooltip = tooltipEl.value
    if (!tooltip) return

    const rect = tooltip.getBoundingClientRect()

    if (x + rect.width > window.innerWidth) {
      x = event.clientX - rect.width - offset
    }

    if (y + rect.height > window.innerHeight) {
      y = event.clientY - rect.height - offset
    }

    tooltipState.x = x
    tooltipState.y = y
  }

  return {
    tooltipEl,
    tooltipState,
    showTooltip,
    moveTooltip,
    hideTooltip
  }
}