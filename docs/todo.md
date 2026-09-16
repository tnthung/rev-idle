

- [x] In capture mode, clicking should not trigger actual UI interaction. Just printing out the
      coordinates and the UI element raycast-ed onto.
- [x] Slot transferring without dragging.
- [x] In capture mode, when click on an element having path, write to clipboard.
- [x] `console.clear()` for clearing the console.
- [x] `rev.read_file(path)` for reading the contents of a file, `null` if not exists.
- [x] `rev.write_file(path, content)` for writing contents to a file.
- [x] Rework transportation layer.
- [x] Replace windows messaging with new transportation layer.
- [x] `rev.delete_file(path)` for deleting a file.
- [x] `rev.press(key)` for simulating a key press.
- [x] `rev.shell(command)` for executing a shell command.
- [x] Rework states module for correct type modeling.
- [x] In-game UI injection.
- [x] Make capture button background Red when in capture mode.
- [x] Change resume icon to point to the right.
- [x] Add lock mode to prevent mis-clicks.
- [x] Add a script list UI for quick access.
- [x] `onDisconnect` hook: called when a disconnection occurs.
- [x] `onConnect` hook: called when a new connection is established.
- [x] `afterLoad` hook: called after the script load/reload, before execution starts.
- [x] `beforeStop` hook: called before the script stops.
- [x] Click on mask should bring the game to foreground.
- [x] Support typescript.
- [ ] Generate state manual when install.
- [ ] Remove `rev.sleep`.
