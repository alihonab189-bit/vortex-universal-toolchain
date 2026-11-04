# VORTEX AI Engine — প্রাকৃতিক ভাষা → কোড

> ১০০% নিজস্ব, rule-based intent engine — **কোনো external LLM/API নেই।**
> অসমীয়া, বাংলা ও English বাক্য শুনে VORTEX কোড তৈরি করে, বাক্যের লিপিতেই
> (অসমীয়া বাক্যে অসমীয়া keyword-এর কোড)।

## ব্যবহার

```bash
# কোড generate
vortexc ai "১ থেকে ১০ পর্যন্ত যোগফল দেখাও"

# কোড + সাথে সাথে চালিয়ে আউটপুট
vortexc ai "৭ এর নামতা" --run
vortexc ai "fibonacci 8" --run
```

## উদাহরণ

| বাক্য | Intent | ফলাফল |
|---|---|---|
| `১ থেকে ১০ পর্যন্ত যোগফল দেখাও` | sum | for-loop + `যোগফল: 55` |
| `sum from 1 to 100` | sum | `sum: 5050` |
| `৭ এর নামতা` | multiplication_table | ৭×১ … ৭×১০ |
| `fibonacci 8` | fibonacci | প্রথম ৮টি সংখ্যা |
| `is 15 even or odd` / `১৫ জোড় না বিজোড়` | even_odd | `odd` |
| `ভোট দিতে পারব কি বয়স ১৯` | vote_eligibility | শর্ত + উত্তর |
| `print "হ্যালো"` / `দেখাও "নমস্কাৰ"` | greet | `show("...")` |

## কীভাবে কাজ করে (নিজস্ব প্রযুক্তি)

1. **লিপি শনাক্তকরণ** — `ৰ/ৱ` দেখলে অসমীয়া, অন্য ভারতীয় লিপি হলে বাংলা,
   নইলে English।
2. **সংখ্যা বের করা** — ASCII ও ভারতীয় অংক `০-৯` — "১ থেকে ১০"-এ শেষ
   সংখ্যাটাই সীমা।
3. **Intent matching** — তিন ভাষার trigger শব্দের টেবিল (যোগফল/sum,
   নামতা/table…)।
4. **Codegen** — বাক্যের লিপি অনুযায়ী keyword (`ধরো/ধরো/let`,
   `লুপ/লুপ/for`, `দেখাও/দেখুৱাওক/show`) আর অংকও সেই লিপিতে (`৪২`)।
5. তৈরি কোড সরাসরি নিজস্ব compiler+VM-এ চালানো যায় (`--run`)।

## কোড → প্রাকৃতিক ভাষা (`explain`)

```bash
vortexc explain examples/loops.vx
```

প্রতিটি statement-এর ব্যাখ্যা: "ভ্যারিয়েবল 'মোট' -এ মান রাখা হয়েছে ০",
"লুপ: 'i' হলে ১ থেকে ১১ পর্যন্ত গুনে ৩ টি কাজ করে"…

## ভবিষ্যৎ (roadmap Phase 5)

নিজস্ব ট্রেনিং-ডেটায় ছোট neural intent model, voice-to-code,
নিজস্ব VM-এ inference engine। ভিত্তি — এই intent engine-ই।
