import { DatabaseSync } from 'node:sqlite';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';

export const ceilingCost=(tokens,price)=>Number((BigInt(tokens)*BigInt(price)+999_999n)/1_000_000n);
export class Refusal extends Error { constructor(code,status=503){super(code);this.code=code;this.status=status;} }
export class Ledger {
 constructor(config){
  this.config=config;fs.mkdirSync(path.dirname(config.dbPath),{recursive:true,mode:0o700});
  if(fs.existsSync(config.dbPath)&&!fs.lstatSync(config.dbPath).isFile())throw new Error('unsafe database file');
  this.db=new DatabaseSync(config.dbPath);fs.chmodSync(config.dbPath,0o600);
  this.db.exec('PRAGMA busy_timeout=5000; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;');
  this.db.exec(`CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);
   CREATE TABLE IF NOT EXISTS calls(id TEXT PRIMARY KEY,digest TEXT NOT NULL,state TEXT NOT NULL CHECK(state IN ('reserved','unknown','complete')),
   reserved INTEGER NOT NULL,accounted INTEGER NOT NULL,created_ms INTEGER NOT NULL,finished_ms INTEGER,input_price INTEGER NOT NULL,output_price INTEGER NOT NULL,
   max_input_tokens INTEGER NOT NULL,max_output_tokens INTEGER NOT NULL,input_tokens INTEGER,output_tokens INTEGER,response TEXT,error_code TEXT);
   CREATE INDEX IF NOT EXISTS calls_state ON calls(state);`);
  // The same balance must not acquire a fresh budget by swapping a token in place.
  const identity=crypto.createHash('sha256').update(config.token).digest('hex');
  this.db.exec('BEGIN IMMEDIATE');try{const row=this.db.prepare("SELECT value FROM metadata WHERE key='provider_identity'").get();
   if(row&&row.value!==identity)throw new Error('provider identity changed');
   this.db.prepare("INSERT OR IGNORE INTO metadata VALUES('provider_identity',?)").run(identity);
   this.db.exec('COMMIT');}catch(e){this.db.exec('ROLLBACK');throw e;}
 }
 totals(now=Date.now()){
  const r=this.db.prepare(`SELECT COALESCE(SUM(accounted),0) lifetime,
   COALESCE(SUM(CASE WHEN state!='complete' OR finished_ms>=? THEN accounted ELSE 0 END),0) daily,
   COALESCE(SUM(CASE WHEN state!='complete' THEN 1 ELSE 0 END),0) unresolved FROM calls`).get(now-86_400_000);
  return r;
 }
 reserve(id,digest,maxOutputTokens,now=Date.now()){
  const c=this.config,reserved=ceilingCost(c.maxInputTokens,c.inputPrice)+ceilingCost(maxOutputTokens,c.outputPrice);
  this.db.exec('BEGIN IMMEDIATE');try{
   const previous=this.db.prepare('SELECT * FROM calls WHERE id=?').get(id);
   if(previous){if(previous.digest!==digest)throw new Refusal('idempotency_conflict',409);if(previous.state!=='complete')throw new Refusal('provider_outcome_unknown');this.db.exec('COMMIT');return {cached:previous.response};}
   const totals=this.totals(now);
   if(totals.unresolved)throw new Refusal('provider_busy_or_unresolved',429);
   if(totals.lifetime+reserved>c.lifetimeBudget||totals.daily+reserved>c.dailyBudget)throw new Refusal('provider_budget_exhausted',429);
   this.db.prepare(`INSERT INTO calls(id,digest,state,reserved,accounted,created_ms,input_price,output_price,max_input_tokens,max_output_tokens)
    VALUES(?,?,'reserved',?,?,?,?,?,?,?)`).run(id,digest,reserved,reserved,now,c.inputPrice,c.outputPrice,c.maxInputTokens,maxOutputTokens);
   this.db.exec('COMMIT');return {reserved};
  }catch(e){this.db.exec('ROLLBACK');throw e;}
 }
 complete(id,{inputTokens,outputTokens,response},now=Date.now()){
  this.db.exec('BEGIN IMMEDIATE');try{
   const row=this.db.prepare('SELECT * FROM calls WHERE id=?').get(id);
   if(!row||row.state!=='reserved')throw new Error('reservation missing');
   if(inputTokens>row.max_input_tokens||outputTokens>row.max_output_tokens)throw new Refusal('provider_usage_exceeds_cap');
   const cost=ceilingCost(inputTokens,row.input_price)+ceilingCost(outputTokens,row.output_price);
   if(cost>row.reserved)throw new Refusal('provider_usage_exceeds_cap');
   this.db.prepare("UPDATE calls SET state='complete',accounted=?,finished_ms=?,input_tokens=?,output_tokens=?,response=? WHERE id=? AND state='reserved'")
    .run(cost,now,inputTokens,outputTokens,response,id);
   this.db.exec('COMMIT');return cost;
  }catch(e){this.db.exec('ROLLBACK');throw e;}
 }
 unknown(id,code){this.db.prepare("UPDATE calls SET state='unknown',error_code=? WHERE id=? AND state='reserved'").run(code,id);}
 close(){this.db.close();}
}
