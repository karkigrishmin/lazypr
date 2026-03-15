Output docs/demo-review.gif
Set Theme "Catppuccin Mocha"
Set Width 1400
Set Height 700
Set FontSize 14
Set Padding 10

Hide
Type "cd /tmp/lazypr-demo"
Enter
Sleep 500ms
Show

# Launch review TUI
Type "/media/grishmin/Work/lazypr/target/release/lazypr review --base main"
Enter
Sleep 2s

# Navigate through files - show priority indicators and semantic summaries
Down
Sleep 800ms
Down
Sleep 800ms
Down
Sleep 800ms
Down
Sleep 800ms

# Switch to diff pane
Tab
Sleep 1s

# Scroll through diff - show syntax highlighting and three-color
Down
Sleep 300ms
Down
Sleep 300ms
Down
Sleep 300ms
Down
Sleep 300ms
Down
Sleep 300ms
Down
Sleep 300ms
Down
Sleep 300ms
Down
Sleep 300ms

Sleep 1s

# Switch back to file tree
Tab
Sleep 500ms

# Search for a file
Type "/"
Sleep 500ms
Type "auth"
Sleep 1s
Escape
Sleep 500ms

# Mark file as viewed
Type "v"
Sleep 1s

# Navigate down
Down
Sleep 500ms

# Skip a file
Type "s"
Sleep 1s

# Show help
Type "?"
Sleep 2s
Type "q"
Sleep 500ms

# Quit
Type "q"
Sleep 500ms
