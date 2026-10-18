import { DynamicStructuredTool } from '@langchain/core/tools';
import { z } from 'zod';
import { CovenantX402Client, requestSchema } from './index.mjs';
import { assert } from './proof.mjs';

/**
 * One application-authorized purchase slot, usable by LangChain and LangGraph.
 * Persist requestId with the application's run before invoking the graph. It is
 * deliberately unrelated to the model's tool-call ID or the graph attempt ID.
 * Recreating this tool with that UUID and the same client journal resumes the
 * original purchase; a changed prompt or generation setting is refused.
 */
export function covenantInferenceTool({ client, requestId, model, maxTokens = 2048, temperature = 0.2, systemPrompt }) {
  assert(client instanceof CovenantX402Client, 'covenant_x402_client_required');
  assert(typeof requestId === 'string' && /^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(requestId), 'trusted_stable_uuid_required');
  // Validate the application-controlled settings now, before tool registration.
  const fixed = requestSchema.parse({ model, max_tokens: maxTokens, temperature,
    messages: [
      ...(systemPrompt === undefined ? [] : [{ role: 'system', content: systemPrompt }]),
      { role: 'user', content: 'configuration validation' },
    ] });
  const fixedSystemPrompt = fixed.messages.length === 2 ? fixed.messages[0].content : undefined;
  const schema = z.object({
    prompt: z.string().min(1).max(64_000).describe('The complete task for the configured inference model. Reuse the identical task when resuming this purchase.'),
  }).strict();

  return new DynamicStructuredTool({
    name: 'covenant_inference',
    description: 'Buy one inference response from Covenant using the application-configured model and USDC spending limits. This tool has one preauthorized purchase identity. Repeating the exact task resumes or returns that purchase; changing the task is refused. Returns a verified answer with payment evidence, or pending. It does not execute the answer or establish its factual correctness.',
    schema,
    responseFormat: 'content_and_artifact',
    func: async args => {
      // Parse again at the dispatch boundary, including direct func callers.
      const { prompt } = schema.parse(args);
      const result = await client.purchase({ requestId, model: fixed.model,
        messages: [
          ...(fixedSystemPrompt === undefined ? [] : [{ role: 'system', content: fixedSystemPrompt }]),
          { role: 'user', content: prompt },
        ], max_tokens: fixed.max_tokens,
        temperature: fixed.temperature });
      const content = result.status === 'completed'
        ? { status: result.status, requestId: result.requestId, text: result.text,
            amountMicroUsdc: result.amountMicroUsdc, transaction: result.settlement.transaction,
            receiptVerified: true }
        : { status: result.status, requestId: result.requestId, requestSha256: result.requestSha256 };
      // ToolMessage.artifact preserves the exact verified result without putting
      // the complete signed receipt/envelope into the planner's message context.
      return [JSON.stringify(content), result];
    },
  });
}
