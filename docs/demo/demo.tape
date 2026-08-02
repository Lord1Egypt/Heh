# VHS tape for the README demo GIF.
#   cd docs/demo && vhs demo.tape
# Shows the three things that make Heh Heh: unbounded integers, infinite
# ranges, and capability-based I/O that can be revoked from outside.

Output ../heh-demo.gif

Set Shell bash
Set FontSize 19
Set Width 1080
Set Height 620
Set Padding 18
Set Theme "Aardvark Blue"
Set TypingSpeed 32ms

Hide
Type "export PS1='$ '" Enter
Type "clear" Enter
Type "alias heh=../../target/release/heh" Enter
Type "clear" Enter
Show

Sleep 700ms

Type "heh --version"
Sleep 400ms
Enter
Sleep 1200ms

Type "cat demo.heh"
Sleep 300ms
Enter
Sleep 3500ms

Type "heh run demo.heh"
Sleep 400ms
Enter
Sleep 3000ms

Type "# 60! exactly. no overflow, no BigInt import."
Sleep 200ms
Enter
Sleep 1600ms

Type "clear"
Enter
Sleep 400ms

Type "cat caps.heh"
Sleep 300ms
Enter
Sleep 3000ms

Type "heh run caps.heh"
Sleep 400ms
Enter
Sleep 1800ms

Type "# now revoke the filesystem from outside:"
Enter
Sleep 1200ms

Type "heh run caps.heh --deny-fs"
Sleep 400ms
Enter
Sleep 2600ms

Type "# capabilities fail closed. 19 keywords. zero dependencies."
Enter
Sleep 2600ms
