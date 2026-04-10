<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="AttributeContributesToRollup">
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    If the document is an ISM_USGOV_RESOURCE and an element meeting
    ISM_CONTRIBUTES specifies attribute @ism:$attrLocalName with a value containing the token
    [$value], then this rule ensures that the ISM_RESOURCE_ELEMENT specifies the attribute
    @ism:$attrLocalName with a value containing the token [$value]. 
  </sch:p>
  <sch:rule id="AttributeContributesToRollup-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:$attrLocalName, ('$value')))]">
    <sch:assert test="util:containsAnyOfTheTokens(@ism:$attrLocalName, ('$value'))" flag="error" role="error">
      <sch:value-of select="$errorMessage"/> 
    </sch:assert>  
  </sch:rule>   
</sch:pattern>