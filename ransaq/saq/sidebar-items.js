window.SIDEBAR_ITEMS = {"constant":[["DEFAULT_USER_AGENT","The HTTP User-Agent used for all requests. This was used as an easy default during development so it is not know whether something that better reflects the intended use would cause requests to be blocked or throttled."]],"fn":[["extract_detailed_info","Traverses through the “Detailed Info” section of the product page to key-value pairs (i.e. “Designation of origin” -> “Mercurey”) which are further processed into a `DetailedInfo` struct."],["extract_linked_data","Finds the JSON-LD `<script>` tag on the page and parses its contents into [`LinkedData`] entries using [`serde_json`]."]],"mod":[["detailed_info","Parsing and cleanup logic to extract data out of the Detailed Info section of product pages."],["linked_data","Just enough JSON-LD/Schema.org support to parse what we need"]],"struct":[["Category","One of the product’s categories."],["Client","Provides a number of methods to interact with the SAQ website"],["ExtractedProduct","Contains all the data extracted from a product page"]]};