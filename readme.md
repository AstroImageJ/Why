# Why, a Java launcher made in Rust

A launcher in Rust to launch Java applications for Windows.

### Features:
- [Configurable JVM lookup](jvm selection.md)
  - If not specified, will try the current working directory (depth of 5)
  - Has configurable fallback to `JAVA_HOME` and common Java installation paths
- Java version validation
- Configuration done through `launcher.ini`

### Drawbacks:
- Only works with `main` method type programs

### Usage
To use Why for your application, you will need to simply rename `JavaLauncher.exe`
to match your application's name and [supply the `launcher.ini` file](launcher.md) 
in your distribution.

You can also use [RCEdit](https://github.com/electron/rcedit) to change the
FileDescription of the launcher, making the process name in Task Manager not have 
".exe" on the end anymore. 

### Why?
- Launch4j does not set process name, and seems to causes issues with Windows UI scaling
- winrun4j can't read Launch4j's config at usertime
- I just wanted something to work