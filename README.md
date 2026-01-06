# ZTE

## Features

- [x] Buffers
- [x] Buffer switching
- [x] Prompt
- [x] Cursor selection
- [x] Basic cursor movement
- [x] Multiple panes
- [x] Pane creation/deletion
- [x] Opener
- [x] Find
- [x] Search in buffer switcher
- [x] File saving
- [x] Syntax highlighting
- [x] Project search (including file path search)
- [x] Auto-indent (and related features)
- [x] Undo/redo
- [x] Ability to resize panes

## Todo

- [ ] Replace
- [ ] Terminal buffers
- [ ] Matching delimiter highlighting
- [ ] Ability to close buffers
- [ ] Tabbed sessions

## Issues to fix

- Switcher search should allow searching whole path, not just parent
- Scroll drag should work in opener preview
- Switching buffers should preserve scroll position
- Allow opening directories immediately into the opener

# Keybindings

## File & buffer manipulation

Buffers represent open files and are tracked independently of view panes.

```
Ctrl + o = Open buffer
Ctrl + s = Save buffer
Ctrl + b = Switch buffer
```

## Search and command prompt

Searching occurs across the repository that the current file resides in.

The command prompt provides access to more fine-grained commands.

```
Ctrl + Shift + o = Path search
Ctrl + Shift + f = Text search

Alt + Return = Open command prompt
```

## Navigation & panes

```
Alt + w = Select pane above
Alt + a = Select pane left
Alt + s = Select pane down
Alt + b = Select pane right

Alt + Shift + w = Create pane above
Alt + Shift + a = Create pane left
Alt + Shift + s = Create pane down
Alt + Shift + d = Create pane right

Alt + q = Close current pane

Alt + Equals = Grow current pane vertically
Alt + Subtract = Shrink current pane vertically
```

## Editing

*'Standard' non-modal editing controls apply, for the most-part*

*The mouse can be used to select and drag as one might expect*

```
Ctrl + l = Go to line
Ctrl + Space = Select token
Ctrl + a = Select all
Ctrl + f = Open finder

PageUp = Scroll upward
PageDown = Scroll downward
Home = Scroll to top
End = Scroll to bottom

Ctrl + z = Undo
Ctrl + y = Redo
Ctrl + c = Copy
Ctrl + x = Cut
Ctrl + v = Paste
Ctrl + / = Comment line or selection
Ctrl + d = Duplicate line or selection
```
