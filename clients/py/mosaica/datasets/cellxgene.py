"""CELLxGENE: the 202,636,280 human and mouse cells of the CZ CELLxGENE Census 2025-11-08, each on
a UMAP of its species' scVI embedding, with the Cell Ontology as a four-tier layer.

Laying the cells out takes a GPU and the Census's 20 GB of embeddings, so this dataset is fetched
from the files `corpora/cellxgene/` prepared and published on Hugging Face, at a pinned revision,
rather than from the Census itself. The cells are in `PARTS` parts, and each part is a uniform
sample of each species. `sample=N` fetches the first N parts. `mosaica.datasets` takes
`DEFAULT_PARTS` when no sample is given and `PARTS` for any sample above it, so each set of parts is
prepared and built in one directory. Every file is checked against its SHA-256 in `SHA256`.

`prepare` writes the files the declaration, `cellxgene.toml`, names: the cells' attributes, each
species' points, the layer's members from each cell type's tier keys, and the vocabularies and tier
names as published. The vocabularies are those of the whole Census, so a value carried by no cell
of a sample is in its vocabulary but is shown to nobody, since every vocabulary is `derived`.

What both this module and `corpora/cellxgene/prepare.py` need is here: the declaration, the
vocabularies and how their blocks are written, and the organisms' names.
"""

from __future__ import annotations

import shutil
from pathlib import Path

REPOSITORY = "mosaicadb/cellxgene-census-map"
#: The commit of `REPOSITORY` the files are fetched from.
REVISION = "e41205a87f13ecf8deae13b7ac810c1be2aad382"
BASE = f"https://huggingface.co/datasets/{REPOSITORY}/resolve/{REVISION}/"

PARTS = 128
DEFAULT_PARTS = 2

DECLARATION = Path(__file__).with_name("cellxgene.toml")

ORGANISM = {"human": "Homo sapiens", "mouse": "Mus musculus"}

#: `(vocabulary, [sources] key, file stem, title, visibility, column of cells.parquet)`. Every one
#: is `derived`, because a value carried only by restricted collections, such as a disease, would
#: otherwise be listed to a viewer who can see none of its cells.
VOCABULARIES = [
    ("cell_type", "cell_type", "cell-type", "Cell type", "derived", "cell_type"),
    ("tissue", "tissue", "tissue", "Tissue", "derived", "tissue"),
    ("tissue_general", "tissue_general", "tissue-general", "Tissue (general)", "derived",
     "tissue_general"),
    ("disease", "disease", "disease", "Disease", "derived", "disease"),
    ("assay", "assay", "assay", "Assay", "derived", "assay"),
    ("sex", "sex", "sex", "Sex", "derived", "sex"),
    ("development_stage", "development", "development-stage", "Development stage", "derived",
     "development_stage"),
    ("self_reported_ethnicity", "ethnicity", "self-reported-ethnicity", "Self-reported ethnicity",
     "derived", "self_reported_ethnicity"),
    ("suspension_type", "suspension", "suspension-type", "Suspension type", "derived",
     "suspension_type"),
    ("organism", "organism", "organism", "Organism", "derived", "organism"),
    ("collection", "collection", "collection", "Collection", "derived", "collection"),
    ("dataset", "dataset", "dataset", "Dataset", "derived", "dataset"),
]

#: The access terms of the 35 collections `corpora/cellxgene/prepare.py` restricts, largest first,
#: as `restricted-collections.json` lists them.
RESTRICTED = tuple(
    f"collection:{uuid}"
    for uuid in """
    f11cb29c-b546-4738-9bd8-66ea621a7bd5 48259aa8-f168-4bf5-b797-af8e88da6637
    854c0855-23ad-4362-8b77-6b1639e7a9fc 77f9d7e9-5675-49c3-abed-ce02f39eef1b
    6f6d381a-7701-4781-935c-db10d30de293 edb893ee-4066-4128-9aec-5eb2b03f8287
    b9fc3d70-5a72-4479-a046-c2cc1ab19efc a0c84e3f-a5ca-4481-b3a5-ccfda0a81ecc
    af893e86-8e9f-41f1-a474-ef05359b1fb7 d86517f0-fa7e-4266-b82e-a521350d6d36
    2a9a17c9-1f61-4877-b384-b8cd5ffa4085 c450e15d-321a-42d6-986b-11409d04896d
    4a2c25af-558a-45fc-bc9a-54ec44a1d63f b0cf0afa-ec40-4d65-b570-ed4ceacc6813
    fbc5881f-1ee3-4ffe-8095-35e15e1a08fc e4c9ed14-e560-4900-a3bf-b0f8d2ce6a10
    120e86b4-1195-48c5-845b-b98054105eec f406a653-c079-4bf9-aab6-85846c27571d
    54004c5c-af08-4693-a606-73871b6ef989 1d1c7275-476a-49e2-9022-ad1b1c793594
    31937775-0602-4e52-a799-b6acdd2bac2e a9254216-6cd8-4186-b32c-349363777584
    a261413d-835b-4f1e-ab0c-dada55ea6afd 367d95c0-0eb0-4dae-8276-9407239421ee
    13d1c580-4b17-4b2e-85c4-75b36917413f c9706a92-0e5f-46c1-96d8-20e42467f287
    6686ada5-43a8-40b6-9a05-6894503606db 3472f32d-4a33-48e2-aad5-666d4631bf4c
    964581c3-b5ac-494d-9bd4-0dcb6fb058da ff668d5d-5b3f-49ee-a007-ff0664bf35ec
    64b24fda-6591-4ce1-89e7-33eb6c43ad7b 99be70c4-e534-4413-a607-ce40b3ff2c77
    558385a4-b7b7-4eca-af0c-9e54d010e8dc 1e313b15-4aca-4e5a-94a3-1093c4c42abd
    ce5b6795-dace-408b-b26b-2ab752c3e20d
    """.split()
)

#: The SHA-256 of each published file at `REVISION`, by name, from the repository's own listing.
SHA256 = dict(
    line.split()
    for line in """
    artifacts-cell-ontology.parquet c4b2ddcefce7a238c0b1332c9c1175ef1fdcb2aae1ffd12390e77137a666ce60
    cell-ontology-tiers.parquet 1a01071f0ebcda681861dcfd6b50ec83dbc04f1ba6f01079a0c9bc8e43ee0e2a
    parts/part-000.parquet ecc4753df50b33778441eb1578999749759460b988d232a84879020cdfaee23d
    parts/part-001.parquet 65469820c0b7db2ae9e62777b8648457bcda0d9cd42a1c4e83babdb47c0784f5
    parts/part-002.parquet a9c36a54556829ebcbaf184e8de6ab049ce13cb768762dd12e1db529760cea10
    parts/part-003.parquet a7ba2bfde871287d66a2039db791619c6af9c72b6be1bc2399bbbdcf82e00d82
    parts/part-004.parquet 81c9865dd816b308c31d66078f93de81c5a4a6bbbcb396832244d110db1027d6
    parts/part-005.parquet 9894fd9ae2258cd2e4d78b640bc49ca67435cce951c4bbef8762aa50ae48f253
    parts/part-006.parquet 30593b909a79b70f6713e01bd6b76270da0d4192af252d29fb00f9487fef8a94
    parts/part-007.parquet 237249ae84e7fd2fffb4d5663ce0dbf88f7965108f401c918d1157a61b63c60c
    parts/part-008.parquet a24d23ab003aaaaef0d9ef9f7d197253c4d46e5ed06a05fce65cb80df05a8837
    parts/part-009.parquet ab3d9ba73e54318dedec7c6c26a507e850a9ffaeebcf15ef51956b1e19cbdc6e
    parts/part-010.parquet 497fa187a04bb48e044aeebc1e75e7a05b8e92b0aa528c16c1f3980dd7e136d4
    parts/part-011.parquet 9a20258d01b2f642ec346e224cfc2e9ec3bc775f0e4c805f68e50db1ea1442db
    parts/part-012.parquet 84063bc935b5a05e4649628d02a998dc3f79b5c0de03c83d90ed7670dd7c36bb
    parts/part-013.parquet 6c5251107053aaadea2abd9fa17eefc34359ed4cd77a012dedc31985791e9ca8
    parts/part-014.parquet 4cef058847eb9431023624ba1e69129a6f46b4768edceccde3c8a681532395a6
    parts/part-015.parquet 5d766c02a8e98bcbcf0479a3f7261df11d7b0160b1e5e190fa9bb8413c10f297
    parts/part-016.parquet a158a1bb9c6d88b4a9982a45f930f2df86a2c0973e1d0b8327049adafcfb56f1
    parts/part-017.parquet 6aa55bd4fd24277455a3127e9d5cfc84bd041056f868c6724d9c9ffd5d59683c
    parts/part-018.parquet db4743ebccb5d4b441aa436c1d1cb6a350abe4d6cb55334829a17075cce5f55e
    parts/part-019.parquet 49204868a9598afb0c5cdf88b00bc230f0bf3862aa1c367d8ee881b47c7f4003
    parts/part-020.parquet 83abad5878413dc9c1b77dd24bd3b01de493c6aa6fd388bfeb796ce1eea0208d
    parts/part-021.parquet 07f26a5c50e22f05d5e40c515db1f7647b5634cbcf91a3e5b05a56f628282639
    parts/part-022.parquet 5975fea5715fb559d4cce7b3ca5489597c483b5104121f4dc6a77fc8f7dfa293
    parts/part-023.parquet 2f39b524003be7c8cbeb4d41de6a9ebe1d315a7876d6915485408944a27947ff
    parts/part-024.parquet fac9c1c48b3b2b44481567e0ca2361be2a251c1f950c768e94d962ba33961817
    parts/part-025.parquet 5e7d37086a4baaf908bdf86a6240689091a165e850e359cd4348bafe09b30182
    parts/part-026.parquet 6f981e5d8cae21f24679ca7c3d049101aa9761bae550170ce6a601758793c939
    parts/part-027.parquet f57dafb1da4e5a3c9704f235cfbc63d007027d6e66b9771c89c93f231358d1c4
    parts/part-028.parquet 5b9cca4e67a807d1b75bc0d0883232bc1db72a1c002882f39020a4ed12815abc
    parts/part-029.parquet 3904c8303cdefddafef33da99337b1510eac73b61fbc9fd9c8ed426581eaa828
    parts/part-030.parquet b423be2158109b81d836a35ed0a0f00aa3ae1133921a7b21fc6d4e6603ada6d2
    parts/part-031.parquet e9ce7825b3b9c4f5f5263cd6f3551619226e60e8c57e7a9fcf5a41a10b6ef4ae
    parts/part-032.parquet b17e6e2d23acffe31b642f9c512156be36fa80fb7ac3fde2f4670eada9f8da78
    parts/part-033.parquet 119bd56758ae043f28bb7c1b2d01f5e8b5cf40db4041a8af0aabf5be7d16c273
    parts/part-034.parquet 07d0b54ca5e266a9ebfb7d126dd50b7d25ffe13a349f4b232c7bc4ad194fd03b
    parts/part-035.parquet 8326ddf72f6b5619f054b909586f7400f51b476706feef69fac35901fba90f4e
    parts/part-036.parquet 906858653c3ddc310f200f05df37f8bab4e4dea5baa4b454294e960f7aae5c07
    parts/part-037.parquet 0b5307101dbf370d8c1772b4d8e65f83049f884286537a627e18ec16cf892de0
    parts/part-038.parquet 75d5bb70bcd903a71272de7f028980904f3e7545a6f09fe9a751626e35f13998
    parts/part-039.parquet 1a6ef3df992634f8514fac4829c6abefb7c6495d90cfeaaeffc6e34e96810587
    parts/part-040.parquet 4300287c70ab904555d6d4394cb84e3ad92cec2a044eed9affc9be4ba227f11a
    parts/part-041.parquet 51ec35cbc07d61b2b2dcdbf7d4d372e560adb8cd670aac0f970d74e86f34adc0
    parts/part-042.parquet 31cfc2c553c5d5b1720ddd49bc197bbd30c8a4f085c3c7613323b47b6eff5a58
    parts/part-043.parquet ca4e9f78a5b613aa06a99fba898825d920d8606ff04a20e8fa1ce7000628b88d
    parts/part-044.parquet 84e58273a569f6d145bd3920c83e569f2284ba26e24ebbce6eac595594bc88fb
    parts/part-045.parquet 2e7b2a1e6c21ba824fdeec7b55c0d46fd984ed40c181c94b3e920931c8379e9b
    parts/part-046.parquet 4b4493c6e3de01fca1c650b65b5a9495c573249541e8fdd5e7c5886fc8c7cdb2
    parts/part-047.parquet 63dc6481d8a02fa9360307ade3bc6650fbbd31f7259ad6283ae68fadecb23d3c
    parts/part-048.parquet 8e8d0b0a8e03cc8adce392e336c3eb3a3ff9b5078b11d60e6acf18feb529232e
    parts/part-049.parquet b1c6a24296ee531c8ff59c8b1e61efdf574dec0eb7a299e9df72e75063463ed6
    parts/part-050.parquet f5155806ccb4b54e386edfd4237491d237a98dc77a35efacc38550b9f2fdea22
    parts/part-051.parquet 79c8d5dcb8f37223a0906087133df53f453e987f05af89512645a2f585b2a89c
    parts/part-052.parquet 3aeafdeed66b00c70bd95e00a4329b13c63c3faddc7db2a1fec35e4a86c2777a
    parts/part-053.parquet ee4b1fa9dcc52fd8ef07899ddb07c889816362b64dd438dfcad8b2817e0aba2d
    parts/part-054.parquet 5bf2f99da746aa7f2f6d061e22af3320d2da440268c8cfad7d64e562175435f3
    parts/part-055.parquet 740f9afa9438ed91480232c3b164e8a122ca3831e4a7a57f5f7b2b1fdb9bb4fa
    parts/part-056.parquet 503981ac48e836934ba2c5d2b0dbe5e7fa3f23bf8ecbe8723bbae16839ade536
    parts/part-057.parquet 914720bfda7a5051f681021961fad4381c9a83e6c330e50d82c61cdebc4a465b
    parts/part-058.parquet 49ef62d1969267ce7978c0a070780407bf87c854e7cb9d5458601fd9ca45db58
    parts/part-059.parquet cf615300402fe548e48a4cc23926e620082c97da410a0a8caed29f0399a5ff4b
    parts/part-060.parquet 0cd0bd712eaa921cfdd507c8876459ca22f38302992e6a50caf18ac35ceec95b
    parts/part-061.parquet ba675ce584369240d6a3eead504c2b6e922fc6bffa015cd87745686d86b5c71c
    parts/part-062.parquet 89bf9d5148f6e228cf5771cc66452ce2ede6d1ce434f6f73d0864be1d96af30f
    parts/part-063.parquet 7ffc9d979b14a9797de4a6d7741dfd8291dd32f20ef8724bdaf9bca7953ed103
    parts/part-064.parquet 89d5bf8683af86866b79b953fd690cfc773ebf4b9c29ff79486fb68ae340e383
    parts/part-065.parquet 2cc8eb941c683250b924760f5e98883970c098d62f4f9cc3fc84fe545bf04ba7
    parts/part-066.parquet 4ef393fc02fc862902344ca2a8d1ccd1011d4f20e5abd2ab92208e02db1c6983
    parts/part-067.parquet a04bfef6ebb1f2548949bc2faf02a0845ad4f4535d8cf5827c2cafb0834f3700
    parts/part-068.parquet b34292a3f4e6abcdabac1105b9ca1bd91672864695441d9f646c5e8c665f245c
    parts/part-069.parquet d52b401bb268967fed36e12abd36003208ebd024d4419da76a9d6e75402fb120
    parts/part-070.parquet a7dd96c5a8abcd58007a6f5cc4fde7dc860fd4ef3367726c79256f69583b2b1c
    parts/part-071.parquet b3f38aa124159cb0de46198d51da89afe86c9f656780e2af0da9cd1b38a40c18
    parts/part-072.parquet 54ee5d891ab8a048054c38ac0d5c23e224df4c98b0a5e48322177ec88a853c9a
    parts/part-073.parquet 7ac3d0e75487065f83a7d26b800f898961db071b817518fafd4aa12e2a7c6bcd
    parts/part-074.parquet 7f4a4059482e1dcd413687801d43118aedcf3639e6b529f5c5e1e1c62757e98b
    parts/part-075.parquet 865068a5c128a0438174d3f32234ab2639bb553877b9ae1fe174edde0dfce153
    parts/part-076.parquet 15b75ca432b4dbe5a64c22b9af1f07575a43bc6e70dd7eb99f0a8f943c89a54b
    parts/part-077.parquet e9ecb616b401cc394e49c2bc791ada095689c5b07d6a1ea3fffe0ed2a5d17a72
    parts/part-078.parquet 7b44ac1aa8e55d773a1f4985243deb8298280fa4cab3d64c4dd92ee63c5b0717
    parts/part-079.parquet 45c6eed2f63d32e4a9942cbcbb624bf837a83e4812860cb372b37a9f729dac1d
    parts/part-080.parquet 807faadf615c5d9cc090c5946d9b6ff0c23bc5f5a51258e24192c296c6c84153
    parts/part-081.parquet b3bb6e0e28df8473f1512cbb323c6ca4e8901fa56e2be1b7fdd87f0d5c71ccb2
    parts/part-082.parquet 58cf0e31f62a1367d3cb0f3f9104b47168053827f9a1185b41f8ae7f3404af81
    parts/part-083.parquet 066a8fc08d4fc2aa5fe51bfe32b53a2de7309a00b489ddd53bc31b43c56c79f4
    parts/part-084.parquet e95d868284346b4a2d32c726df378977250e79875dd7dc3e44fc97565f5cc432
    parts/part-085.parquet a8e13359c90baed278342cdbdb30224a35ebe21c21526d8365348e0cd403ead4
    parts/part-086.parquet f3e60b73d71fd376e20d6c44887a859af6d42bcf17ea06ac56ddc2f4ad95df78
    parts/part-087.parquet 0ed08091c622c3343b9e58f08dc048b1c3e320fa9f281bef60d9c06eadf7c882
    parts/part-088.parquet ee67b5039fc56f6b77f004e9f6e80e6669b29a60f5788fe907da53a7f023e98b
    parts/part-089.parquet 6d81ac6edd2fb598f7e90397273e4ca5810f19570081b8b87ca82f64651a15f0
    parts/part-090.parquet 46b10007311d6a83cd47fcd6866fb32d5a7bd54622b4e36cbc677c11ba34b933
    parts/part-091.parquet b4589c363a9ff2eea95c9c77bab882b7b37ec0f6fad2271ab24911ccd179ee06
    parts/part-092.parquet 7da7621efbaf62df22907a052fd3d5d1f1dbed7bdb1e637b94f39026cc0a9e32
    parts/part-093.parquet aef00158905ab35c78eb4e48b29def637ddf68cf7a23b46e8203f503770fb6af
    parts/part-094.parquet 938512e4d2612124151d586cb2415ae7556b08fcdf482716f5a9f64e22f73e90
    parts/part-095.parquet 700b5ac4010a52e9c8b430aa5adc27508f68646cd468f52ceaf79760585384a9
    parts/part-096.parquet 52bc5b8f4e97dd510dd27878c386c12d74e165a6f1193d4fafa9bce32e5ebd93
    parts/part-097.parquet 3c45ddc26f94cbeceb688adaf23bc6d9c520d93fbba2139a97816568d666d1a6
    parts/part-098.parquet d5edbafe724e4efbf7e41bea68e6b3b8996f7ed262fd00e4a0054789c21f0833
    parts/part-099.parquet 887a04529195785ba959e906cfdf660af30c86402af5de8ac6397b650b976534
    parts/part-100.parquet 697bc057746feed5573aaf1fb2f9dcc489cda598ffb9469d6077fcc0b3f9d0d3
    parts/part-101.parquet 496c982f9f211693fea857f985abe73761f126ae0762d4d430a45c152e8eb4f2
    parts/part-102.parquet 67a86432ac9feb82a633fd9f12dfd7ee26537f36d4f031abd8deca01f58bd427
    parts/part-103.parquet 79ecdbbf57bc2ac0b96da532aa89a56c43878bf426237019966b72e425fd9938
    parts/part-104.parquet 9e8ff8517df5d6b642148107676629440b5193a7df0fed0c9240da4f18429864
    parts/part-105.parquet f55f087dce219f6d0fb8127b78398dafa4580cb654d711b9e2dd62411ee6f04c
    parts/part-106.parquet 331cd82c7678c465ec32b6d750a8bcbb2ff17b83504a3c5c525a637622ecc0f0
    parts/part-107.parquet 20f2b18f1f6735e0bbe01f9b7aeed74ec3c663ca532246534236d1db8ddd6617
    parts/part-108.parquet e4523f0d9527f3ffaec4d975b009ee90ab0e7bcae5f03138ece72c33bd400078
    parts/part-109.parquet 3eb0103e2aafcbb28dbbc57841881e5bcc3f4395d44cd3e617f779a58807597f
    parts/part-110.parquet 9a7a9a776f0c59fd06d91cf038c8fabdc0a3d00dcda9aa85ea851bbcb444f890
    parts/part-111.parquet dd822bb70efc7842f0509d37049b4d77f5035f98450793f19bb3305f47db5970
    parts/part-112.parquet 4d92d885d2c45bb14b96c8c9e184abc78ef5961ce71361a7a0364a27a08e6d0e
    parts/part-113.parquet cd64b495bcd77ec4c4c8113fbbc817f5694aa9e353855a5922a26234a0e2e5ee
    parts/part-114.parquet 0b497a1c1882c2ff9aa817e5183af91ea7855bd9f344732d4e7f6b2742b023ac
    parts/part-115.parquet fb7d500c95e1923b0160810cd7a649c0fd0b0b2d546c9502c0dad88c4ffbfc39
    parts/part-116.parquet 55338268b2cfc729edbef13e82c484293c1d60d7988d408a32afac56a6042d73
    parts/part-117.parquet 716516948f50324c0f9d775bf00310463df1fda563bb35cf3e88b6e81a6156a2
    parts/part-118.parquet 5aa3bc0b2f6942f9fec01a2a9280aa1f6d5387bb3f1289afcc541b34b2e07054
    parts/part-119.parquet f2cbb9460c76a4a51431073a99784c17e5651d3de39563ecafc6665a723fab64
    parts/part-120.parquet dd6f3806cdedc1f6577e434d2a10b3f7fb24f15fabeb0971a5ecda80aecd47fe
    parts/part-121.parquet 8ec99400043e9750c95a16b8e32c0101670994a7f6254a30ed9e3f3cb41eaf70
    parts/part-122.parquet 70d1bb81126e091308e67c8ce1b1e6f283425903051f602b50471e31cb3cebec
    parts/part-123.parquet cfb3d78ed44f6af9f6df84ce37e8ad53b894710f898714e6db4ddbf1f085a5d9
    parts/part-124.parquet e12d6e97129f33dcf16729f4340324a2c3ee4824cd41356bd49bdf7b7988f92a
    parts/part-125.parquet 331e9995b02522dfdb4937c7a7601fe5e9c5e1d8fef5f615b5185112b0caad48
    parts/part-126.parquet 6290ea9b8915ea3720077ec085490599cb4b9bdec07494671135f4e9900a7270
    parts/part-127.parquet cc1acd664fcca6a8e35e72238eb777aa53d621b561445596eaefd67273c6fda2
    restricted-collections.json f9183177d51154014bafe8d37bdfe45b51a6b1247215e40864b6ad977b4189f0
    vocab-assay.parquet 2a4f92cf7498a0dc5824b6a69b6864c7cc224c1c51b70490174bf27af7c6dd17
    vocab-cell-type.parquet 5482ba057fe7eb39d4357ee438259637d43078a6dd777bd400430fd8f6fd2075
    vocab-collection.parquet 390d4f6b14759c0d49b6770a8cc20821efcc4e1a2e8b1f832f23fd297afe4074
    vocab-dataset.parquet ca68c403d2d1d44bb53fd893046ab8e9e87158f4712563818734f17c4899cdfd
    vocab-development-stage.parquet ca7a2b1a84591c4fff4e20352e33d9448f37c4f28b40370f670e5cecce579088
    vocab-disease.parquet d9a4b1fd3c49f20003476ac49e63e40643404c56ba9fe02982998817e01c1d88
    vocab-organism.parquet 868086e5046541d531f5d54ea8b79b202ce095575f4a8ab350a01d284c5cfbc0
    vocab-self-reported-ethnicity.parquet d31bd4aee59a647638804db3128edf73d74e3606507dc862c5a5cc7a30162076
    vocab-sex.parquet 176c7d133fa0a8c1db1fed7f7eed134796c3074832c0f63c071017ad9855af12
    vocab-suspension-type.parquet af3cbfb6e9aec6f14c33aa36586bfe4a8412bb25fd274a622a3bf33659497485
    vocab-tissue-general.parquet e7f0804033bf7aac6ceabf43cf8e7a6243fd452641dc90843d89fa7f07968c13
    vocab-tissue.parquet 400f2fa68d8c3c1f74a14b2d635e73d0c21916fbf3442c6067c3bbc10a11742c
    """.strip().splitlines()
)


#: The published files besides the parts.
TIERS = "cell-ontology-tiers.parquet"
ARTIFACTS = "artifacts-cell-ontology.parquet"
RESTRICTIONS = "restricted-collections.json"
SMALL = [
    TIERS,
    ARTIFACTS,
    RESTRICTIONS,
    *(f"vocab-{stem}.parquet" for _, _, stem, *_ in VOCABULARIES),
]


def vocabulary_toml(sizes: dict[str, int]) -> str:
    """The `[[vocabulary]]` blocks, each as wide as the code space its `sizes` entry needs."""
    from ._steps import width

    blocks = []
    for name, key, _, title, visibility, _ in VOCABULARIES:
        n = sizes[name]
        blocks.append(
            "[[vocabulary]]\n"
            f'name       = "{name}"\n'
            f'title      = "{title}"  # {n:,} keys\n'
            f'width      = "{width(n)}"\n'
            'value_set  = "closed"\n'
            f'visibility = "{visibility}"\n'
            f'source     = "{key}"'
        )
    return "\n\n".join(blocks)


def write_declaration(path: Path, sizes: dict[str, int]) -> None:
    """`cellxgene.toml` at `path`, its `# <vocabularies>` line replaced by the blocks."""
    from ._steps import declared

    declared(DECLARATION, "# <vocabularies>", vocabulary_toml(sizes), path)


def parts(sample: int) -> list[str]:
    """The names of the first `sample` parts."""
    return [f"parts/part-{i:03d}.parquet" for i in range(sample)]


def fetch(raw: Path, progress: bool = True, sample: int = DEFAULT_PARTS) -> list[Path]:
    """Download the small files and the parts `sample` reads into `raw`, each once, and refuse
    them where the restricted collections they list are not `RESTRICTED`, which the personas
    hold."""
    import json

    from .._refusal import Refusal
    from . import _steps

    names = SMALL + parts(sample)
    paths = _steps.fetch(raw, {name: BASE + name for name in names}, progress, SHA256)
    listed = json.loads((raw / RESTRICTIONS).read_text(encoding="utf-8"))
    published = {entry["term"] for entry in listed.get("terms", [])}
    if published != set(RESTRICTED):
        raise Refusal(
            f"fetch: {RESTRICTIONS} lists {len(published)} restricted collections that differ "
            f"from the {len(RESTRICTED)} this package's personas hold; set "
            "mosaica.datasets.cellxgene.RESTRICTED to the terms it lists"
        )
    return paths


def prepare(raw: Path, out: Path, sample: int = DEFAULT_PARTS) -> None:
    """Write the files `cellxgene.toml` names into `out`, from the parts `sample` reads."""
    import duckdb
    import pyarrow.parquet as pq

    con = duckdb.connect()
    con.execute("SET enable_progress_bar = false")
    con.execute(f"SET temp_directory = '{out / '.duckdb'}'")
    listed = ", ".join(f"'{raw / name}'" for name in parts(sample))
    con.execute(f"CREATE VIEW cells AS SELECT * FROM read_parquet([{listed}])")

    def copy(query: str, name: str) -> None:
        con.execute(f"COPY ({query}) TO '{out / name}' (FORMAT parquet, COMPRESSION zstd)")

    copy("SELECT * EXCLUDE (x, y, access) FROM cells", "cells.parquet")
    for species, organism in ORGANISM.items():
        copy(
            "SELECT entity_id, x::DOUBLE AS x, y::DOUBLE AS y, access FROM cells "
            f"WHERE organism = '{organism}'",
            f"points-{species}.parquet",
        )
    copy(
        f"SELECT c.entity_id AS entity, t.key FROM cells c "
        f"JOIN read_parquet('{raw / TIERS}') t USING (cell_type)",
        "members-cell-ontology.parquet",
    )
    cells = con.execute("SELECT count(*) FROM cells").fetchone()[0]
    members = pq.ParquetFile(out / "members-cell-ontology.parquet").metadata.num_rows
    if members != cells:
        raise ValueError(
            f"{cells - members:,} of {cells:,} cells have a cell type that {TIERS} does not "
            "list; fetch the files again"
        )
    con.close()
    shutil.rmtree(out / ".duckdb", ignore_errors=True)

    sizes = {}
    for name, _, stem, *_ in VOCABULARIES:
        shutil.copyfile(raw / f"vocab-{stem}.parquet", out / f"vocab-{stem}.parquet")
        sizes[name] = pq.ParquetFile(out / f"vocab-{stem}.parquet").metadata.num_rows
    shutil.copyfile(raw / ARTIFACTS, out / ARTIFACTS)
    write_declaration(out / "schema.toml", sizes)
