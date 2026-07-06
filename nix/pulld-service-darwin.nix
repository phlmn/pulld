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
        command = lib.getExe serviceCfg.package; # use this over serviceConfig.Program as it wait for the nix store to be available
        serviceConfig = {
          StandardErrorPath = "${logDir}/launchd-stderr.log";
          StandardOutPath = "${logDir}/launchd-stdout.log";
          KeepAlive = true;
          RunAtLoad = true;
          ExitTimeOut = 30 * 60;
          ThrottleInterval = 10;
          EnvironmentVariables = serviceCfg.environment;
          WorkingDirectory = "/var/root";
        };
        inherit (serviceCfg) path;
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
