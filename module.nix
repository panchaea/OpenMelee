{ config, lib, pkgs, ... }:

let
  cfg = config.services.openmelee;
  inherit (lib) mkOption mkEnableOption mkIf types;
in {
  options = {
    services.openmelee = {
      enable = mkEnableOption "Whether to enable OpenMelee.";

      package = mkOption {
        type = types.package;
        default = pkgs.openmelee;
        description = "OpenMelee package to use";
      };

      user = mkOption {
        type = types.str;
        default = "openmelee";
        description = "User account under which OpenMelee runs.";
      };

      group = mkOption {
        type = types.str;
        default = "openmelee";
        description = "Group account under which OpenMelee runs.";
      };

      extraGroups = mkOption {
        type = types.listOf types.str;
        default = [ ];
        description =
          "List of extra groups to which the 'openmelee' user belongs.";
      };

      webserverListenAddr = mkOption {
        type = types.str;
        default = "127.0.0.1";
        example = "0.0.0.0";
        description = "IP for the OpenMelee web server to listen on.";
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
        description = "IP for the OpenMelee matchmaking server to listen on.";
      };

      jwtSecretPath = mkOption {
        type = types.str;
        example = "/run/secrets/openmelee_jwt_secret";
        description = ''
          Path to a file containing the OpenMelee JWT secret.
        '';
      };

      extraEnv = mkOption {
        type = types.attrsOf types.str;
        default = { };
        example = { OPENMELEE_MATCHMAKING_MAX_PEERS = "2048"; };
        description = "Extra environment variables for the OpenMelee service.";
      };

      workDir = mkOption {
        type = types.str;
        default = "/var/lib/openmelee";
        description = "Working directory for the OpenMelee service.";
      };
    };
  };

  config = mkIf cfg.enable {
    environment.systemPackages = [ pkgs.openmelee ];

    ids.gids.openmelee = 470;
    ids.uids.openmelee = 470;

    users.users.${cfg.user} = {
      uid = config.ids.uids.openmelee;
      group = cfg.group;
      extraGroups = cfg.extraGroups;
      home = cfg.workDir;
      createHome = true;
      useDefaultShell = true;
    };

    users.groups.${cfg.group}.gid = config.ids.gids.openmelee;

    systemd.services.openmelee = {
      description = "OpenMelee matchmaking server";
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];
      environment = {
        OPENMELEE_WEBSERVER_ADDRESS = cfg.webserverListenAddr;
        OPENMELEE_WEBSERVER_PORT = builtins.toString cfg.webserverListenPort;
        OPENMELEE_MATCHMAKING_SERVER_ADDRESS = cfg.matchmakingServerListenAddr;
        OPENMELEE_DATABASE_URL = "${cfg.workDir}/openmelee.sqlite";
        OPENMELEE_JWT_SECRET_PATH = cfg.jwtSecretPath;
      } // cfg.extraEnv;

      serviceConfig = {
        User = cfg.user;
        ExecStart = "${cfg.package}/bin/openmelee";
        Restart = "always";
        WorkingDirectory = cfg.workDir;
      };
    };
  };
}
