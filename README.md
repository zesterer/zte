# ZTE

## Features

- [x] Buffers
- [x] Buffer switching
- [x] Prompt
- [x] Cursor selection
- [x] Basic cursor movement
- [x] Multiple panes
- [x] Pane creation/deletion
- [x] Open
- [x] Save
- [x] Save as
- [x] Move
- [x] Find
- [x] Search in buffer switcher
- [x] Syntax highlighting
- [x] Project search (including file path search)
- [x] Regex search
- [x] Auto-indent (and related features)
- [x] Undo/redo
- [x] Ability to resize panes
- [x] Terminal buffers
- [x] Tabs
- [x] Mouse support
- [x] Delimiter matching

## Todo

- [ ] Replace
- [ ] Git integration
- [ ] LSP integration

## Issues to fix

- Return should only complete block if matching delim has different ident

# Keybindings

## File & buffer manipulation

Buffers represent open files and are tracked independently of view panes.

```
Ctrl + n = New buffer
Ctrl + o = Open buffer
Ctrl + s = Save buffer
Ctrl + Shift + s = Save buffer as
Ctrl + m = Move buffer
Ctrl + b = Switch to buffer
Ctrl + q = Close buffer

Ctrl + t = New terminal pane (WIP)
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
Escape = Cancel current task or close editor

Alt + w = Select pane above
Alt + a = Select pane left
Alt + s = Select pane down
Alt + b = Select pane right

Alt + Shift + w = Create pane above
Alt + Shift + a = Create pane left
Alt + Shift + s = Create pane down
Alt + Shift + d = Create pane right

Alt + PageUp = Select tab above
Alt + PageDown = Select tab below

Alt + Shift + PageUp = Create tab above
Alt + Shift + PageDown = Create tab below

Alt + q = Close current pane

Alt + Equals = Grow current pane vertically
Alt + Subtract = Shrink current pane vertically
```

## Editing

*'Standard' non-modal editing controls apply, for the most-part*

*The mouse can be used to select and drag as one might expect*

```
Ctrl + l = Go to line
Ctrl + Space = Select containing block
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

## Search regex

When searching, a non-standard regex is supported in which:

- Any inline whitespace matches any inline whitespace in the search
- An inline regex expression can be included with `@(...)`. For example, `foo@([dl])` will match `food` and `fool`
