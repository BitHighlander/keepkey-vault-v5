declare module 'coinselect' {
  interface Input {
    txId: string;
    vout: number;
    value: number;
    script?: string;
  }

  interface Output {
    address?: string;
    value?: number;
  }

  interface Result {
    inputs: Input[];
    outputs: Output[];
    fee: number;
  }

  function coinSelect(utxos: Input[], outputs: Output[], feeRate: number): Result;
  export default coinSelect;
}

declare module 'coinselect/split' {
  interface Input {
    txId: string;
    vout: number;
    value: number;
    script?: string;
  }

  interface Output {
    address?: string;
    value?: number;
  }

  interface Result {
    inputs: Input[];
    outputs: Output[];
    fee: number;
  }

  function coinSelectSplit(utxos: Input[], outputs: Output[], feeRate: number): Result;
  export default coinSelectSplit;
} 