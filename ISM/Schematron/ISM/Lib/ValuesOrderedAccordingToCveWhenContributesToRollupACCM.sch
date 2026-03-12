<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="ValuesOrderedAccordingToCveWhenContributesToRollupACCM">
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    For values that contributed to rollup, the values are ordered according to its CVE.
    
    To perform sorting, each attribute token
    is converted into a numerical value based on its characters. Next, each attribute token is 
    given an order number, which compares its position to that of its value in the CVE file.
    Next, each order number is compared to that of its previous sibling to determine if the tokens
    are in order. If a token is found whose order number is less than that of its previous sibling, 
    0 is returned for its sorted order number. If a token's order number is greater than that of its 
    previous sibling, 1 is returned. If two tokens have the same order number, their original attribute
    values are compared. If the original attribute value contains numbers then the comparison is made 
    on its converted numerical value; otherwise, the comparison is made on its string value. If an 
    attribute value is found whose value is less than that of its previous sibling,  0 is returned
    for its sorted order number; otherwise 2 is returned. Finally, if any tokens are found with 0 as 
    its sorted order number, then the rule fails as those tokens are out of order.
    
    For values that do not contribute to rollup, the values are ordered alphabetically.
  </sch:p>
  <sch:rule id="ValuesOrderedAccordingToCveWhenContributesToRollupACCM-R1" context="*[$ISM_USGOV_RESOURCE and @ism:$attrLocalName]">
    <sch:assert test="if ($contributesToRollup) then count(tokenize(util:unsortedValues(@ism:$attrLocalName, $cveTermList),' '))=0 else true()" flag="error" role="error">
      <sch:value-of select="$includedInRollUpErrorMessage"/>
      The following values [<sch:value-of select="util:unsortedValues(@ism:$attrLocalName, $cveTermList)"/>] for [<sch:value-of select="@ism:$attrLocalName"/>] that contribute to rollup are out of order with respect to its CVE.
    </sch:assert>  
    <sch:assert test="if (not($contributesToRollup)) then count(tokenize(util:unorderedValues($nonACCMAttrValuesTok, $nonACCMCveTermListTok),' '))=0 else true()" flag="error" role="error">
      <sch:value-of select="$excludedFromRollUpNonACCMErrorMessage"/>
      The following non-ACCM values [<sch:value-of select="util:unorderedValues($nonACCMAttrValuesTok, $nonACCMCveTermListTok)"/>] for [<sch:value-of select="@ism:$attrLocalName"/>] that does not contribute to rollup are out of order with respect to its CVE.
    </sch:assert>
    <sch:assert test="if (not($contributesToRollup)) then count(tokenize(util:nonalphabeticValues($ACCMAttrValuesTok),' '))=0 else true()" flag="error" role="error">
      <sch:value-of select="$excludedFromRollUpACCMErrorMessage"/>
      The following ACCM values [<sch:value-of select="util:nonalphabeticValues($ACCMAttrValuesTok)"/>] for [<sch:value-of select="@ism:$attrLocalName"/>] that does not contribute to rollup are not in the expected alphabetical order.
    </sch:assert>
    <sch:assert test="if (not($contributesToRollup)) then count(tokenize(util:relativeOrderBetweenACCMAndNonACCMWhenExcludeFromRollup($attrValues),' '))=0 else true()" flag="error" role="error">
      <sch:value-of select="$excludedFromRollUpACCMRelativeLocationErrorMessage"/>
      The following non-ACCM values [<sch:value-of select="util:relativeOrderBetweenACCMAndNonACCMWhenExcludeFromRollup($attrValues)"/>] for [<sch:value-of select="@ism:$attrLocalName"/>] that does not contribute to rollup are not in the correct relative order to the ACCM values [<sch:value-of select="util:getStringFromSequence($ACCMAttrValuesTok)"/>].
      The ACCM values exist between the LEFT set of non-ACCMs [<sch:value-of select="$nonACCMLeftSetTok"/>] and the RIGHT set of non-ACCMs [<sch:value-of select="$nonACCMRightSetTok"/>].
    </sch:assert>
  </sch:rule>
</sch:pattern>