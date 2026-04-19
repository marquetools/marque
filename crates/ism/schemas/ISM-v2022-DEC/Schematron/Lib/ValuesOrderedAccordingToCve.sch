<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="ValuesOrderedAccordingToCve">
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    To perform sorting, each attribute token is converted into a numerical value based on its characters. 
    Next, each attribute token is given an order number, which compares its position to that of its value in the CVE file.
    Next, each order number is compared to that of its previous sibling to determine if the tokens
    are in order. If a token is found whose order number is less than that of its previous sibling, 
    0 is returned for its sorted order number. If a token's order number is greater than that of its 
    previous sibling, 1 is returned. If two tokens have the same order number, their original attribute
    values are compared. If the original attribute value contains numbers then the comparison is made 
    on its converted numerical value; otherwise, the comparison is made on its string value. If an 
    attribute value is found whose value is less than that of its previous sibling,  0 is returned
    for its sorted order number; otherwise 2 is returned. Finally, if any tokens are found with 0 as 
    its sorted order number, then the rule fails as those tokens are out of order. 
  </sch:p>
  <sch:rule id="ValuesOrderedAccordingToCve-R1" context="*[($ISM_USGOV_RESOURCE or $ISM_USCUIONLY_RESOURCE) and @ism:$attrLocalName]">
    <!-- Convert each character to a numerical value, then concatenate the results to form a number-string -->
      <sch:assert test="count(tokenize(util:unsortedValues(@ism:$attrLocalName, $cveTermList),' ')) = 0" flag="error" role="error">
         <sch:value-of select="$errorMessage"/>
      The following values are out of order [<sch:value-of select="util:unsortedValues(@ism:$attrLocalName, $cveTermList)"/>] for [<sch:value-of select="@ism:$attrLocalName"/>]
    </sch:assert>
  </sch:rule>
</sch:pattern>