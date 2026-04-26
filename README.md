# WFAutoSplitter

**WFAutoSplitter** is an automatic run tracker and splitter for Warframe speedrunners.
It reads the game's log file in real time, detects run start, splits, and finish —
then saves everything to a local database for analysis.

## How It Works

The program monitors Warframe's log file and reacts to specific lines based on
user-defined **templates**. Each template describes the structure of a run:
mission start trigger, split points with custom names, and the finish condition.
Once a template is activated, WFAutoSplitter automatically tracks every run
that matches it.

## Features

- **Real-time log parsing** — detects run state instantly without any manual input
- **Custom templates** — flexible configuration of start, splits, and finish triggers
  for any mission or route
- **Millisecond precision** — timing accurate to milliseconds, unlike the in-game
  timer which only shows seconds
- **Load time removal** — option to exclude time spent between mission loads,
  so only actual gameplay time is counted
- **Trigger-only mode** — correctly measures run time even for missions where
  the in-game timer is not displayed
- **Run history** — every completed run is saved to a local database with full
  split data
- **Statistics & charts** — view your progress over time with run history and
  performance graphs
- **Overlay** — an optional always-on-top overlay displays current timer and
  split information during a run. The overlay is completely optional and does
  not affect run recording in any way
- **Auto-update** — the app checks for new versions on startup and notifies you
  when an update is available

## Getting Started

1. Download the latest [installer](../../releases)
2. Install and launch WFAutoSplitter
3. Point the app to your Warframe log file (`EE.log`)
4. Create or import a template for the mission you want to track
5. Activate the template and start running
