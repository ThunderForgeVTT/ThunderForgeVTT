export interface SetupProvider {
  provider_key: string;
  display_name: string;
}

export interface SetupStatus {
  setup_required: boolean;
  setup_completed: boolean;
  configured_oauth_providers: SetupProvider[];
}
