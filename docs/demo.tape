Output assets/agenttrace-demo.gif
Set Shell "bash"
Set FontSize 18
Set Width 1400
Set Height 900
Set Padding 16
Set Framerate 24
Set Theme "Dracula"

Type "go build -o /tmp/agenttrace-demo ./cmd/agenttrace && /tmp/agenttrace-demo --demo"
Enter
Sleep 2s
Type "1"
Sleep 1s
Down
Down
Sleep 800ms
Type ":"
Type "health <80"
Enter
Sleep 1s
Type "2"
Sleep 1s
Type "w"
Sleep 1s
Type "4"
Sleep 1s
Type "0"
Sleep 1s
Type "q"
