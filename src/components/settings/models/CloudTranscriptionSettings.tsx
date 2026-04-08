import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { CloudTranscriptionProvider } from "@/bindings";
import { commands } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { useModelStore } from "@/stores/modelStore";
import { ProviderSelect } from "../PostProcessingSettingsApi/ProviderSelect";
import { ApiKeyField } from "../PostProcessingSettingsApi/ApiKeyField";
import { ModelSelect } from "../PostProcessingSettingsApi/ModelSelect";
import type { ModelOption } from "../PostProcessingSettingsApi/types";
import { Button } from "../../ui/Button";

const PROVIDER_KEYS: CloudTranscriptionProvider[] = ["openai", "groq", "gemini"];

export const CloudTranscriptionSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();
  const refreshModelsList = useModelStore((s) => s.loadModels);
  const loadCurrentModel = useModelStore((s) => s.loadCurrentModel);

  const [modelOptions, setModelOptions] = useState<ModelOption[]>([]);
  const [modelsLoading, setModelsLoading] = useState(false);

  const selectedProvider = settings?.cloud_transcription_provider ?? "groq";

  const providerKey = useMemo(() => {
    if (settings?.selected_model === "cloud-openai") return "openai";
    if (settings?.selected_model === "cloud-groq") return "groq";
    if (settings?.selected_model === "cloud-gemini") return "gemini";
    return selectedProvider;
  }, [settings?.selected_model, selectedProvider]);

  const apiKey =
    settings?.cloud_transcription_api_keys?.[providerKey] ?? "";
  const modelId =
    settings?.cloud_transcription_models?.[providerKey] ?? "";

  const providerDropdownOptions = useMemo(
    () =>
      PROVIDER_KEYS.map((id) => ({
        value: id,
        label: t(`settings.models.cloudModels.providers.${id}`),
      })),
    [t],
  );

  const fetchListedModels = useCallback(async () => {
    const key = settings?.cloud_transcription_api_keys?.[providerKey] ?? "";
    if (!key.trim()) {
      setModelOptions([]);
      return;
    }
    setModelsLoading(true);
    try {
      const result = await commands.fetchCloudTranscriptionModels(
        providerKey as CloudTranscriptionProvider,
        key,
      );
      if (result.status === "ok") {
        setModelOptions(
          result.data.map((id) => ({ value: id, label: id })),
        );
      } else {
        setModelOptions([]);
      }
    } finally {
      setModelsLoading(false);
    }
  }, [providerKey, settings?.cloud_transcription_api_keys]);

  useEffect(() => {
    void fetchListedModels();
  }, [fetchListedModels]);

  const handleProviderChange = async (value: string) => {
    const p = value as CloudTranscriptionProvider;
    const r = await commands.changeCloudTranscriptionProviderSetting(p);
    if (r.status === "ok") {
      await refreshSettings();
      await refreshModelsList();
      await loadCurrentModel();
    }
  };

  const handleApiKeyBlur = async (value: string) => {
    const trimmed = value.trim();
    await commands.changeCloudTranscriptionApiKeySetting(providerKey, trimmed);
    await refreshSettings();
    void fetchListedModels();
    await refreshModelsList();
    await loadCurrentModel();
  };

  const handleModelSelect = async (value: string) => {
    const trimmed = value.trim();
    await commands.changeCloudTranscriptionModelSetting(
      providerKey,
      trimmed,
    );
    await refreshSettings();
  };

  const handleModelCreate = async (value: string) => {
    await handleModelSelect(value);
  };

  const handleRefreshClick = () => {
    void fetchListedModels();
  };

  return (
    <div className="rounded-xl border border-mid-gray/20 bg-mid-gray/5 p-4 space-y-4">
      <div>
        <h2 className="text-sm font-medium text-text/80">
          {t("settings.models.cloudModels.title")}
        </h2>
        <p className="text-sm text-text/50 mt-1">
          {t("settings.models.cloudModels.description")}
        </p>
      </div>

      <div className="flex flex-col gap-1">
        <span className="text-xs font-medium text-text/60">
          {t("settings.models.cloudModels.providerLabel")}
        </span>
        <ProviderSelect
          options={providerDropdownOptions}
          value={providerKey}
          onChange={(v) => void handleProviderChange(v)}
        />
      </div>

      <div className="flex flex-col gap-1">
        <span className="text-xs font-medium text-text/60">
          {t("settings.models.cloudModels.apiKeyLabel")}
        </span>
        <ApiKeyField
          value={apiKey}
          onBlur={(v) => void handleApiKeyBlur(v)}
          disabled={false}
        />
      </div>

      <div className="flex flex-col gap-1">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-xs font-medium text-text/60">
            {t("settings.models.cloudModels.modelLabel")}
          </span>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            onClick={handleRefreshClick}
            disabled={modelsLoading || !apiKey.trim()}
          >
            {t("settings.models.cloudModels.refreshModels")}
          </Button>
        </div>
        <ModelSelect
          value={modelId}
          options={modelOptions}
          disabled={false}
          placeholder={t("settings.models.cloudModels.customModelPlaceholder")}
          isLoading={modelsLoading}
          onSelect={(v) => void handleModelSelect(v)}
          onCreate={(v) => void handleModelCreate(v)}
          onBlur={() => {}}
        />
      </div>

    </div>
  );
};
