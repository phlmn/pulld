{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.pulld;

  serviceModule = lib.types.submodule {
    options = {
      enable = lib.mkEnableOption "Enable this service.";

      package = lib.mkPackageOption pkgs "pulld" { };

      environment = lib.mkOption {
        default = { };
        type = lib.types.attrsOf lib.types.str;
        example = lib.literalExpression ''
          {
            PULLD_BACKEND = "github";
            PULLD_OWNER = "phlmn";
            PULLD_REPO = "pulld";
          }
        '';
        description = "pulld config environment variables";
      };

      user = lib.mkOption {
        type = lib.types.str;
        default = null;
        example = "root";
        description = ''
          User for the systemd service.
        '';
      };

      extraGroups = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        example = [ "podman" ];
        description = ''
          Additional groups for the systemd service.
        '';
      };

      path = lib.mkOption {
        type = lib.types.listOf lib.types.package;
        default = [ ];
        example = [ "" ];
        description = ''
          Additional packages that should be added to the runners's `PATH`.
        '';
      };

      environmentFile = lib.mkOption {
        type = lib.types.listOf lib.types.path;
        default = [ ];
        example = [ "/var/secrets/pulld.env" ];
        description = ''
          File to load environment variables
          from. This is helpful for specifying secrets.
        '';
      };
    };
  };

  mkService = name: serviceCfg: {
    name = "pulld-${name}";
    value = {
      description = "pulld Service - ${name}";
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      serviceConfig = {
        DynamicUser = serviceCfg.user == null;
        User = serviceCfg.user;
        SupplementaryGroups = serviceCfg.extraGroups;
        EnvironmentFile = serviceCfg.environmentFile;
        ExecStart = lib.getExe serviceCfg.package;
        ExecReload = "kill -TERM $MAINPID";
        Restart = "on-failure";
        RestartSec = 15;
        TimeoutStopSec = 30 * 60;
      };
      restartIfChanged = false; # allows self updates
      reloadIfChanged = true;
      inherit (serviceCfg) environment path;
    };
  };
in
{
  options = {
    services.pulld = lib.mkOption {
      default = { };
      type = lib.types.attrsOf serviceModule;
      example = lib.literalExpression ''
        {
          nixos-config = {
            environment = {
              PULLD_BACKEND = "github";
              PULLD_OWNER = "phlmn";
              PULLD_REPO = "nixos-config";
            };

            user = "root";
          };
        }
      '';
      description = "pulld configurations";
    };
  };

  config = {
    systemd.services =
      let
        mkServices = lib.mapAttrs' mkService;
        enabledServices = lib.filterAttrs (_: service: service.enable) cfg;
      in
      mkServices enabledServices;
  };
}
