import { useState, useEffect } from 'react';
import type { ComponentSchema, ComponentMetadata } from '../types';
import { getErrorMessage } from '../types';
import { getComponentSchema, getComponentMetadata } from '../tauri/commands';
import { Toast } from '../components/atoms';

export interface UseComponentDetailResult {
  schema: ComponentSchema | null;
  metadata: ComponentMetadata | null;
  loading: boolean;
  schemaError: string | null;
  metadataError: string | null;
}

export function useComponentDetail(digest: string | undefined): UseComponentDetailResult {
  const [schema, setSchema] = useState<ComponentSchema | null>(null);
  const [metadata, setMetadata] = useState<ComponentMetadata | null>(null);
  const [loading, setLoading] = useState(true);
  const [schemaError, setSchemaError] = useState<string | null>(null);
  const [metadataError, setMetadataError] = useState<string | null>(null);

  useEffect(() => {
    if (!digest) {
      setLoading(false);
      return;
    }

    let active = true;
    setLoading(true);
    setSchema(null);
    setMetadata(null);
    setSchemaError(null);
    setMetadataError(null);

    Promise.allSettled([
      getComponentSchema(digest),
      getComponentMetadata(digest),
    ]).then(([schemaResult, metaResult]) => {
      if (!active) return;

      if (schemaResult.status === 'fulfilled') {
        setSchema(schemaResult.value);
      } else {
        const reason = getErrorMessage(schemaResult.reason);
        setSchemaError(reason);
        Toast.error(`Failed to load component schema: ${reason}`);
      }

      if (metaResult.status === 'fulfilled') {
        setMetadata(metaResult.value);
      } else {
        const reason = getErrorMessage(metaResult.reason);
        setMetadataError(reason);
        Toast.error(`Failed to load component metadata: ${reason}`);
      }

      setLoading(false);
    });

    return () => {
      active = false;
    };
  }, [digest]);

  return { schema, metadata, loading, schemaError, metadataError };
}
