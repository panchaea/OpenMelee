# OpenMelee

Open-source reimplementation of the Slippi matchmaking server.

## Motivation

https://github.com/project-slippi/Ishiiruka/issues/191

## Building this project

``` sh
$ cd openmelee
$ cargo build --release
$ OPENMELEE_JWT_SECRET_PATH=/path/to/file ./target/release/openmelee
```

or, with [Nix](https://nixos.org/):

```sh
$ cd openmelee
$ nix build
$ OPENMELEE_JWT_SECRET_PATH=/path/to/file ./result/bin/openmelee
```

## Testing

At the moment, one must build the latest version of [Ishiiruka](https://github.com/project-slippi/Ishiiruka) with `ishiiruka.patch` applied, since the endpoints for matchmaking and user discovery are not configurable. Such a build is provided when running `nix develop`.

## About Slippi

- Here, "Slippi" refers to the [Ishiiruka](https://github.com/project-slippi/Ishiiruka) fork of Dolphin. In other words, the emulator, not the launcher ("Slippi Desktop").

- When starting matchmaking, Slippi queries an [HTTP server](https://github.com/project-slippi/Ishiiruka/blob/v2.5.1/Source/Core/Core/Slippi/SlippiUser.h#L47) for the current user's information (connect code, name, and last played Slippi version) based on the content of the `uid` field in [`user.json`](https://github.com/project-slippi/Ishiiruka/blob/v2.5.1/Source/Core/Core/Slippi/SlippiUser.cpp#L113).
  
  If Slippi cannot retrieve the user's information, they will be redirected to https://slippi.gg to register.
  
- The matchmaking communication itself uses [`enet`](http://enet.bespin.org/), again connecting to a [fixed server](https://github.com/project-slippi/Ishiiruka/blob/v2.5.1/Source/Core/Core/Slippi/SlippiMatchmaking.h#L72).
  
  The messages are JSON, just with ENet headers. There are only a few message types, as can be seen [here](https://github.com/project-slippi/Ishiiruka/blob/v2.5.1/Source/Core/Core/Slippi/SlippiMatchmaking.cpp#L16).
