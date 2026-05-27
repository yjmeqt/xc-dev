# HelloDemo — xc-dev test fixture

This is a minimal iOS app fixture used in `xc-dev`'s integration tests.
It is **not** a real app — it exists solely to give `xcodebuild -list -json`
and `xcodebuild -showBuildSettings -json` something real to talk to.

## What's here

| File / Dir | Purpose |
|---|---|
| `HelloDemo.xcodeproj/` | Generated Xcode project (committed; no `xcodegen` needed at test time) |
| `HelloDemo/HelloDemoApp.swift` | One-file SwiftUI stub (the sole source file) |
| `project.yml` | `xcodegen` spec used to regenerate the `.xcodeproj` if needed |

## Generating / regenerating

If you need to update the project file, install [xcodegen](https://github.com/yonaskolb/XcodeGen)
and run:

```bash
cd examples/HelloDemo
xcodegen generate
```

The `.xcodeproj` is intentionally tracked in git so tests work on any machine
that has `xcodebuild` — no `xcodegen` required at CI time.

## What the integration test checks

`xc-dev/tests/init_fixture.rs` copies this directory into a temp dir and runs
`xc-dev :proj init --yes`. It asserts that the generated `.xc-dev/project.toml`
contains:

- `path = "HelloDemo.xcodeproj"`
- `scheme = "HelloDemo"`
- `bundle_id = "com.example.HelloDemo"`
- `product_name = "HelloDemo"`
