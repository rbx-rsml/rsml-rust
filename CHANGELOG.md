# 1.1.0
- Fixed tweens not being compiled correctly.
- the `typechecker` is now conditionally enabled via the `typechecker` feature.
- the `compiler` is now conditionally enabled via the `compiler` feature.

# 1.0.2
- Changed tweens to use `Attributes`.

# 1.0.1
- Renamed the `Assignment` macro return context to `Datatype`

# 1.0.0
## Changes
- Completely rewrote library:
    - The parsing step now generates an AST.
    - Added a new compilation step which turns the parsed AST into StyleSheet nodes (effectively what the parsing step in the old version did).
- Removed the `@name` declaration.
- Added a typechecker component to the library.
    - An error will be thrown when calling a macro which doesn't exist.
    - An error will be thrown when a macro overload is invalid.
    - An error will be thrown if a function's args are incorrect.
    - An error will be thrown when referencing a token which doesn't exist.
    - An errror will be thrown when a infinite recursion cycle is detected when calling a macro.
    - The typechecker can run in an optional `strict` mode. This mode can be enabled by adding `--!strict` at the top of an rsml file, or via `languageMode` in a luaurc. Strict mode requires a property to exist on **every** class in a multi-class selector.
- We now parse numbers as f64 to mitigate precision issues, particularly with the `font` function.

# 0.1.14
- Added query selector support to the lexer and parser.

# 0.1.13
- Added `@tween` declaration support to the lexer and parser.

# 0.1.12
- Strings starting with "rbxassetid://" are now properly accepted in the font tuple annotation.

# v0.1.8
- You can now multiply A Rect with a Vector2 or a Vector2int16.
- Fixed issue where Vector2int16's and Vector3int16's could not be floored, ceiled, rounded, or absoluted.

# v0.1.7
- Fixed issue with udim2 when specifying 4 components of the tuple.

# v0.1.6
- fixed issues with certain property modifer tuples not working properly on all types

# v0.1.5
- Added more builtins.
- Changed Enum datatypes to use `EnumItem`.

# v0.1.4
- Changed `properties` from `HashMap` to `Attributes` in `TreeNode`.

# v0.1.3
- Fixed issues with `TreeNodeGroup`

# v0.1.2
- Fixed issues with `TreeNodeGroup`

# v0.1.1
- The derives are now also returned from `file_to_rsml`.

# v0.1.0
- Optimized how multiline strings are parsed.
- Properties, attributes and selectors can now have dashes in the middle of them.
- Numbers can now have underscores in them (parity with luau).
- Tailwind colors defined with no shade will now default to shade `500` instead of `50`.
- Added functions for lexing and parsing utils (@util) - lex_rsml_utils & parse_rsml_utils.
- Added skin color presets - 4 undertones (rose, peach, gold and olive), and 11 shades (50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950). They can be defined via the `skin:{undertone}:{shade}` syntax.
- Performing a mathemetical operation between 2 empty tuples will now result in `None` instead of `0`.
- Fixed issue with mathematical operations panicking due to overflows.
- Implemented statics ($!).
- Added `oklch` and `oklab` tuple annotations.
- All of the built in color presets (tw, css, bc, skin) have been changed to be in the oklab color space. The main noticeable change will be improved lerping.
- Added `abs` tuple annotation - calculates the absolute values for a datatypes component's.
- Fixed infinite recursion error when subtracting a non udim or udim2 from a number.
- Added macros.
- derives are now processed in a separate pre-pass.
- you can now derive a tuple of paths via the derive pre pass.
- fixed issue with dividing and multiplying a px.
- Added a new `file_to_rsml` helper function.
