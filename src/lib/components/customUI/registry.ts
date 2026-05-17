import type { Component } from 'svelte';
import JsonInspector from './JsonInspector.svelte';

export type CustomUIProps = {
  props: Record<string, unknown>;
  onRespond: (response: Record<string, any>) => void;
};

export const customUIRegistry: Record<string, Component<CustomUIProps>> = {
  'json-inspector': JsonInspector as Component<CustomUIProps>
};

export function resolveCustomUI(component: string): Component<CustomUIProps> | undefined {
  return customUIRegistry[component];
}
