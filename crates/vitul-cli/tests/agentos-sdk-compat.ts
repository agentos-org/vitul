// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

// This fixture is copied into a generated app for its TypeScript and build checks.
import {
  isHarnessKitSubagentModelSelectedEvent,
  type AppModel,
  type AppModelRegistration,
  type AppModelUpdate,
  type HarnessKitDecisionModelConfig,
  type HarnessKitRunRequest,
  type HarnessKitSkillSelection,
  type ModelInfo
} from 'agentos-sdk-ts'
import { createAgentOSClient } from './lib/agentos'

export async function checkHarnessKitSdkCompatibility() {
  const sdk = createAgentOSClient()
  const skills = await sdk.harnesskit.listSkills()
  const selection: HarnessKitSkillSelection = {
    preferred: skills.map(({ skillId }) => ({ skillId })),
    autoSelect: true
  }
  const decision: HarnessKitDecisionModelConfig = {
    modelId: 'decision-model',
    toolSelectionEnabled: true,
    contextWindowSize: 8192
  }
  const request: HarnessKitRunRequest = {
    requestId: 'request',
    goal: 'Review the project',
    llmConfig: { model: 'chat-model' },
    decisionModelConfig: decision,
    skillSelection: selection,
    skillPolicy: { allowAutomaticSelection: true }
  }
  const run = await sdk.harnesskit.startRun('thread', request)
  const branch = await sdk.harnesskit.forkThread({
    sourceThreadId: 'thread',
    threadId: 'branch',
    throughTurnId: 'turn',
    title: 'Branch'
  })
  const events = await sdk.harnesskit.events('thread', { runId: run.runId })
  for await (const event of events) {
    if (isHarnessKitSubagentModelSelectedEvent(event)) {
      const reason: string | undefined = event.data.reason
      return { branch, reason }
    }
  }
  return { branch }
}

// These APIs are available only to the built-in Harness app on the host.
export async function checkHostOnlyHarnessKitTypes() {
  const sdk = createAgentOSClient()
  const settings = await sdk.harnesskit.getSkillSettings()
  const workspaces = await sdk.harnesskit.listWorkspaces()
  return { settings, workspaces }
}

export async function checkModelKitOutputTokenTypes() {
  const sdk = createAgentOSClient()
  const registration: AppModelRegistration = {
    name: 'chat-model',
    contextWindowSize: 32768,
    maxOutputTokens: 4096
  }
  const update: AppModelUpdate = { maxOutputTokens: 2048 }
  const registered = await sdk.modelkit.registerAppModel({
    request: registration
  })
  if (registered.id) {
    await sdk.modelkit.updateAppModel({
      modelId: registered.id,
      request: update
    })
  }
  const models: ModelInfo[] = await sdk.modelkit.listModels()
  const appModels: AppModel[] = await sdk.modelkit.listAppModels()
  const modelLimit: number | undefined = models[0]?.maxOutputTokens
  const appModelLimit: number | undefined = appModels[0]?.maxOutputTokens
  return { modelLimit, appModelLimit }
}

const invalidModel: AppModelRegistration = {
  name: 'chat-model',
  // @ts-expect-error The output-token limit must be a number.
  maxOutputTokens: '4096'
}
void invalidModel

const invalidDecision: HarnessKitDecisionModelConfig = {
  modelId: 'decision-model',
  // @ts-expect-error The SDK requires a boolean for tool selection.
  toolSelectionEnabled: 'yes'
}
void invalidDecision
