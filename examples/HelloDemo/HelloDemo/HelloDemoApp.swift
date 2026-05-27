// HelloDemo — xc-dev test fixture. See ../README.md.
//
// This file exists only so the Xcode project has a target with sources;
// `xcodebuild -list -json` and `-showBuildSettings -json` (the only commands
// our tests run) resolve project metadata without compiling Swift.
import Foundation

enum HelloDemo {
    static let greeting = "Hello, demo"
}
