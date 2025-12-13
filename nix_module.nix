{config, lib, pkgs, ...}:

with lib;
let
	cfg = config.services.break-enforcer;
in
{
	options = {
		services.break-enforcer = {
			enable = mkEnableOption "break-enforcer";
		};
	};

	config = mkIf cfg.enable {
		systemd.services.break-enforcer = {
			description = "Disables input during breaks";
			after = ["network.target"];
			wantedBy = ["multi-user.target"];

			serviceConfig = {
				Type = "simple";
				ExecStart = ''
					${pkgs.break-enforcer}/bin/break-enforcer run \
					--work-duration 25:00 \
					--break-duration 05:00 \
					--break-start-lead 30s \
					--break-end-lead 5s \
					--break-start-notify audio \
					--break-end-notify audio \
					--tcp-api
					'';
			};
		};
	};
}

# {
# 	# options = {
# 	# 	enabled = mkOption {
# 	# 		type = types.bool;
# 	# 		default = true;
# 	# 		description = "a test";
# 	# 	}
# 	# }
#
# }
