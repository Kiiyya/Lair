# A package manager for Idris2

Intends to be a cargo-like package manager for Idris2.

Example manifest (TOML format, `Egg.toml`):
```toml
[package]
name = "AmazingTool"
version = "0.1.0"

[dependencies]
CoolCollections = { git = "https://github.com/Kiiyya/CoolCollections" }
NotJson = { git = "https://github.com/Kiiyya/NotJson" }
```

AmazingTool depends on both CoolCollections and NotJson.
NotJson depends on CoolCollections.
In AmazingTool, we can then use our dependency like this (`src/AmazingTool.idr`):
```idr
module AmazingTool

import NotJson

main : IO ()
main = do
  case parse "Whoops" of -- `parse` comes from NotJson
    Just json => putStrLn "Success"
    Nothing => putStrLn "Failed to parse"
```

Which we can then run as follows:
```
lair run
```
```
Downloading CoolCollections from https://github.com/Kiiyya/CoolCollections
Downloading NotJson from https://github.com/Kiiyya/NotJson
Building CoolCollections
1/2: Building CoolCollections.SimpleMap (build/deps/CoolCollections/src/CoolCollections/SimpleMap.idr)
2/2: Building CoolCollections (build/deps/CoolCollections/src/CoolCollections.idr)
Building NotJson
1/1: Building NotJson (build/deps/NotJson/src/NotJson.idr)
Building AmazingTool
1/1: Building AmazingTool (src/AmazingTool.idr)
```

## How it works
All dependencies are cloned into `./build/deps/*`, where they are built.
There is no concept such as *installing* idris2 packages, all you have to do is add your
dependencies to the `[dependencies]` section in the manifest.

## Project and namespace structure
Have a look at [CoolCollections](https://github.com/Kiiyya/CoolCollections).
In short:
```
CoolCollections/ -- Repo root
	Egg.toml -- The manifest
	src/
		CoolCollections.idr -- Main file, `CoolCollections` idris2 module.
		CoolCollections/
			SimpleMap.idr -- `CoolCollections.SimpleMap` idris2 module.
```
Also this
[discussion](https://discord.com/channels/827106007712661524/841274390481600562/921754399334875156)
on Discord may be relevant.

## Todo
A lot is yet to be done, most of which should be fairly straightforward, since we already use
the `git2` library.
There is also stub code for tracing, which should hopefully make logging what is happening easier
and prettier.

- Prevent dependency cycles, either by maintaining a `depth` field in each dependency node, or
  actually building the dependency graph.
- Better error handling (currently too many `unwrap`s and `anyhow::Error`s)
- Better status information of what is currently happening.
- Show idris2 errors nicer.
- Implement local dependencies (currently only git repos).
- Allow for specific git hash or git tag as dependency.
- Find a better name.

