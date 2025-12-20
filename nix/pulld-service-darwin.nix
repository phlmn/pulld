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
          User for the service.
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
    };
  };

  mkService = name: serviceCfg:
    let
      logDir = "/var/log/pulld/${name}";
    in
    {
      name = "pulld-${name}";
      value = {
        serviceConfig = {
          UserName = serviceCfg.user;
          Program = lib.getExe serviceCfg.package;
          StandardErrorPath = "${logDir}/launchd-stderr.log";
          StandardOutPath = "${logDir}/launchd-stdout.log";
          KeepAlive = true;
          RunAtLoad = true;
          ExitTimeOut = 30 * 60;
          EnvironmentVariables = serviceCfg.environment;
        };
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
    launchd.daemons =
      let
        mkServices = lib.mapAttrs' mkService;
        enabledServices = lib.filterAttrs (_: service: service.enable) cfg;
      in
      mkServices enabledServices;
  };
}
