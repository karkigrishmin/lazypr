Output docs/demo-ghost.gif
Set Theme "Catppuccin Mocha"
Set Width 1000
Set Height 500
Set FontSize 14
Set Padding 10

Hide
Type "cd /tmp/lazypr-demo"
Enter
Sleep 500ms
Show

# Ghost analysis
Type "/media/grishmin/Work/lazypr/target/release/lazypr ghost --base main"
Enter
Sleep 3s

# Split plan
Type "/media/grishmin/Work/lazypr/target/release/lazypr split --base main --dry-run"
Enter
Sleep 3s

# Impact analysis
Type "/media/grishmin/Work/lazypr/target/release/lazypr impact src/hooks/useAuth.ts"
Enter
Sleep 3s
