# Why, a Java launcher made in Rust

A launcher in Rust to launch Java applications for Windows, Linux, and MacOS. Since 2.0, 
made to be compatible with JPackage's app format, but with support for merging the app config 
bundled with the app with one in a global location so things such as memory settings can
persist reinstalls without overriding any changes to the launch config provided in the app.

### Features:
- [Configurable JVM lookup](<jvm selection.md>)
  - If not specified, will try the current working directory (depth of 5)
  - Has configurable fallback to `JAVA_HOME` and common Java installation paths
- Java version validation
- Configuration done through jpackage's launcher config

### Drawbacks:
- Only works with programs that have a `main` method

### Usage
To use Why for your application, you will need to simply rename `JavaLauncher[.exe]` 
to `YourApplication[.exe]`, and replace the launcher jpackage uses by default. 
The most reliable way to do this is to package the app in two steps, 
the first creating the app image, then replacing the launcher, 
then creating the bundle for final distribution.

The configuration overlay has the [same paths as the global jpackage config](https://bugs.openjdk.org/browse/JDK-8287060),
but with `_Overlay` appended to the end, e.g. `AstroImageJ_Overlay.cfg`. Options are merged together with the config 
bundled with the app, with the overlay taking precedence.

You can also use [RCEdit](https://github.com/electron/rcedit) to change the
FileDescription of the launcher, making the process name in Task Manager not have 
".exe" on the end anymore. 

### Why?
- Launch4j does not set process name, and seems to causes issues with Windows UI scaling
- winrun4j can't read Launch4j's config at usertime
- JPackage configs support overriding the entire config, but not specific settings.