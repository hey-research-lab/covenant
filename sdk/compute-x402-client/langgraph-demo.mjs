// A persistent, model-selected support workflow. No provider key or wallet is
// embedded here. The application supplies the planner and bounded paid client.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { StateGraph, MessagesAnnotation, START, END } from '@langchain/langgraph';
import { ToolNode } from '@langchain/langgraph/prebuilt';
import { HumanMessage, SystemMessage, ToolMessage } from '@langchain/core/messages';
import { SqliteSaver } from '@langchain/langgraph-checkpoint-sqlite';
import { covenantInferenceTool } from './langchain.mjs';
import { assert, sha256, stable } from './proof.mjs';

export const SAMPLE_TICKET = 'Sample support ticket: I switched to the annual plan yesterday, but my billing page shows both a monthly charge and an annual charge. I also cannot download the invoice. Our finance team needs it by Friday. Can you help?';
export const SUPPORT_DRAFT_POLICY = 'This is a draft for a human support agent to review, not an action on a customer account. You have not accessed any account, flagged a case, escalated a ticket, issued a refund, restored access, or contacted anyone. Never claim those actions happened. Do not promise a reply or action today or confirm that a charge is a duplicate. Acknowledge the reported problem, ask for the evidence needed, and say that a human billing agent can review it. Keep the customer reply to two or three sentences.';
const INSTRUCTIONS = 'You are a support triage coordinator handling a labeled sample ticket. You have one preauthorized paid specialist tool, covenant_inference. Decide whether to use it to produce the triage and draft reply. If you use it, give it the complete sample ticket and ask for: category, priority with a short reason, evidence to request, and a concise empathetic draft reply for a human support agent to review. Do not invent account access, a confirmed duplicate charge, a refund, a promised resolution time, or a verified cause. The prompt must fit 3000 UTF-8 bytes. After the tool returns a completed result, summarize the useful outcome without making further tool calls. Keep the answer grounded in the sample, and label the reply as a draft.';

const save = (file, value) => writeFileSync(file, JSON.stringify(value, null, 2) + '\n', { mode: 0o600 });
const viewMessage = message => ({ type: message.getType(), content: message.content,
  ...(message.tool_calls?.length ? { tool_calls: message.tool_calls } : {}),
  ...(message instanceof ToolMessage ? { tool_call_id: message.tool_call_id, name: message.name, artifact: message.artifact } : {}),
  ...(message.usage_metadata ? { usage: message.usage_metadata } : {}) });

/**
 * Execute each phase in a new process with the same private output directory.
 * plan: model selects the tool; checkpoint before any paid tool execution.
 * purchase: resume that checkpoint and perform the single bounded purchase.
 * replay: re-execute the saved tool node, reusing the completed client journal.
 * finish: resume after the tool and produce the planner's final draft summary.
 */
export async function runSupportDemo({ client, planner, requestId, threadId, model,
  outputDirectory, phase, ticket = SAMPLE_TICKET }) {
  assert(['plan', 'purchase', 'replay', 'finish'].includes(phase), 'invalid_demo_phase');
  assert(typeof threadId === 'string' && threadId.length > 0, 'stable_thread_id_required');
  const directory = resolve(outputDirectory); mkdirSync(directory, { recursive: true, mode: 0o700 });
  const tool = covenantInferenceTool({ client, requestId, model, maxTokens: 2048, temperature: 0.2,
    systemPrompt: SUPPORT_DRAFT_POLICY });
  const checkpointer = SqliteSaver.fromConnString(join(directory, 'graph.sqlite'));
  const identityPath = join(directory, 'run.json');
  const identity = { requestId, threadId, model, ticketSha256: sha256(ticket), version: 1 };
  if (phase === 'plan') {
    // Never silently start a fresh purchase or overwrite another run.
    writeFileSync(identityPath, JSON.stringify(identity, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  } else assert(stable(JSON.parse(readFileSync(identityPath, 'utf8'))) === stable(identity), 'demo_identity_changed');
  const selectedPlanner = planner.bindTools([tool], { tool_choice: 'auto' });
  const graph = new StateGraph(MessagesAnnotation)
    .addNode('agent', async state => {
      const hasResult = state.messages.some(message => message instanceof ToolMessage);
      const response = await (hasResult ? planner : selectedPlanner).invoke([new SystemMessage(INSTRUCTIONS), ...state.messages]);
      if (hasResult) assert(!response.tool_calls?.length, 'one_purchase_per_demo_run');
      else if (response.tool_calls?.length) {
        assert(response.tool_calls.length === 1 && response.tool_calls[0].name === tool.name, 'unexpected_tool_selection');
        tool.schema.parse(response.tool_calls[0].args);
      }
      return { messages: [response] };
    })
    .addNode('tools', new ToolNode([tool], { handleToolErrors: false }))
    .addEdge(START, 'agent')
    .addConditionalEdges('agent', state => state.messages.at(-1).tool_calls?.length ? 'tools' : END)
    .addEdge('tools', 'agent')
    .compile({ checkpointer, ...(phase === 'plan' ? { interruptBefore: ['tools'] } : {}),
      ...(phase === 'purchase' || phase === 'replay' ? { interruptAfter: ['tools'] } : {}) });
  try {
    let config = { configurable: { thread_id: threadId }, recursionLimit: 6 };
    // A pending response is also a completed graph node. Re-enter the saved
    // pre-tool checkpoint to poll the same purchase, never the planner node.
    if (phase === 'purchase' || phase === 'replay') config = JSON.parse(readFileSync(join(directory, 'before-tool.json'), 'utf8'));
    if (phase === 'finish') {
      const accepted = JSON.parse(readFileSync(join(directory, 'purchase.json'), 'utf8')).purchase;
      const latest = await graph.getState(config);
      const observation = latest.values.messages.findLast(message => message instanceof ToolMessage);
      assert(accepted?.status === 'completed' && observation?.artifact?.status === 'completed' &&
        stable(observation.artifact) === stable(accepted), 'cannot_finish_pending_purchase');
    }
    const startedAt = new Date().toISOString();
    const result = await graph.invoke(phase === 'plan' ? { messages: [new HumanMessage(ticket)] } : null, config);
    const snapshot = await graph.getState({ configurable: { thread_id: threadId } });
    const record = { phase, startedAt, finishedAt: new Date().toISOString(), identity,
      next: snapshot.next, messages: result.messages.map(viewMessage), checkpoint: snapshot.config };
    if (phase === 'plan') {
      assert(snapshot.next.includes('tools'), 'model_did_not_select_paid_tool');
      save(join(directory, 'before-tool.json'), { ...snapshot.config, recursionLimit: 6 });
    }
    if (phase === 'purchase' || phase === 'replay') {
      const observation = result.messages.findLast(message => message instanceof ToolMessage);
      assert(observation?.artifact?.status === 'completed', 'purchase_pending_resume_original_checkpoint');
      record.purchase = observation.artifact;
      if (phase === 'replay') {
        const accepted = JSON.parse(readFileSync(join(directory, 'purchase.json'), 'utf8')).purchase;
        assert(stable(record.purchase) === stable(accepted), 'replayed_result_changed');
      }
    }
    if (phase === 'finish') assert(snapshot.next.length === 0, 'graph_not_finished');
    save(join(directory, phase + '.json'), record);
    return record;
  } finally { checkpointer.db.close(); }
}
