{ config, lib, pkgs, ... }:

let
  cfg = config.services.slippi-re;
  inherit (lib) mkOption mkEnableOption mkIf types;
in {
  options = {
    services.slippi-re = {
      enable = mkEnableOption "Whether to enable slippi-re.";

      package = mkOption {
        type = types.package;
        default = pkgs.slippi-re;
        description = "slippi-re package to use";
      };

      user = mkOption {
        type = types.str;
        default = "slippi-re";
        description = "User account under which slippi-re runs.";
      };

      group = mkOption {
        type = types.str;
        default = "slippi-re";
        description = "Group account under which slippi-re runs.";
      };

      extraGroups = mkOption {
        type = types.listOf types.str;
        default = [ ];
        description =
          "List of extra groups to which the 'slippi-re' user belongs.";
      };

      webserverListenAddr = mkOption {
        type = types.str;
        default = "127.0.0.1";
        example = "0.0.0.0";
        description = "IP for the slippi-re web server to listen on.";
      };

      webserverListenPort = mkOption {
        type = types.int;
        default = 5000;
        example = 8080;
        description = "Port for the web server to listen on.";
      };

      matchmakingServerListenAddr = mkOption {
        type = types.str;
        default = cfg.webserverListenAddr;
        example = "0.0.0.0";
        description = "IP for the slippi-re matchmaking server to listen on.";
      };

      extraEnv = mkOption {
        type = types.attrsOf types.str;
        default = { };
        example = { SLIPPI_RE_MATCHMAKING_MAX_PEERS = "2048"; };
        description = "Extra environment variables for the slippi-re server.";
      };

      workDir = mkOption {
        type = types.str;
        default = "/var/lib/slippi-re";
        description = "Working directory for the slippi-re service.";
      };
    };
  };

  config = mkIf cfg.enable {
    environment.systemPackages = [ pkgs.slippi-re ];

    ids.gids.slippi-re = 469;
    ids.uids.slippi-re = 469;

    users.users.${cfg.user} = {
      uid = config.ids.uids.slippi-re;
      group = cfg.group;
      extraGroups = cfg.extraGroups;
      home = cfg.workDir;
      createHome = true;
      useDefaultShell = true;
    };

    users.groups.${cfg.group}.gid = config.ids.gids.slippi-re;

    systemd.services.slippi-re = {
      description = "slippi-re matchmaking server";
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];
      environment = {
        SLIPPI_RE_WEBSERVER_ADDRESS = cfg.webserverListenAddr;
        SLIPPI_RE_WEBSERVER_PORT = builtins.toString cfg.webserverListenPort;
        SLIPPI_RE_MATCHMAKING_SERVER_ADDRESS = cfg.matchmakingServerListenAddr;
        SLIPPI_RE_DATABASE_URL = "${cfg.workDir}/slippi-re.sqlite";
      } // cfg.extraEnv;

      serviceConfig = {
        User = cfg.user;
        ExecStart = "${cfg.package}/bin/slippi-re";
        Restart = "always";
        WorkingDirectory = cfg.workDir;
      };
    };
  };
}
