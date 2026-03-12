<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00104">
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    [ISM-ID-00104][Error] If the document is an ISM_USGOV_RESOURCE and any
    element in the document is: 
      1. Unclassified and meets ISM_CONTRIBUTES 
        AND 
      2. Has the attribute @ism:nonICmarkings containing [SBU-NF] 
        AND
      3. The ISM_RESOURCE_ELEMENT has attribute @ism:nonICmarkings does not contain [XD] or [ND] 
        AND
      4. The ISM_RESOURCE_ELEMENT has attribute @ism:disseminationControls does not contain [NF]
    Then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [SBU-NF]. 
    
    Human Readable: USA Unclassified documents having SBU-NF and not having XD, ND, or explicit Foreign Disclosure and
    Release markings must have SBU-NF at the resource level.
  </sch:p>
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    If the document is Unclassified and is an ISM_USGOV_RESOURCE, the current
    element is the ISM_RESOURCE_ELEMENT, some element meeting ISM_CONTIBUTES specifies attribute
    @ism:nonICmarkings with a value containing the token [SBU-NF], and the attribute @ism:nonICmarkings
    on the ISM_RESOURCE_ELEMENT does not contain the token [XD] or [ND], and the attribute 
    @ism:disseminationControls on the resource element does not contain the token [NF]; 
    this rule ensures sure that ISM_RESOURCE_ELEMENT specifies 
    attribute @ism:nonICmarkings with a value containing the token [SBU-NF].</sch:p>
  
  <sch:rule id="ISM-ID-00104-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and $bannerClassification = 'U' and index-of($partNonICmarkings_tok, 'SBU-NF') &gt; 0 and not(util:containsAnyOfTheTokens(string-join(@ism:nonICmarkings, ' '), ('XD', 'ND'))) and not(util:containsAnyOfTheTokens(string-join(@ism:disseminationControls, ' '), ('NF')))]">
    <sch:assert test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('SBU-NF'))" flag="error" role="error">
      [ISM-ID-00104][Error] If the document is an ISM_USGOV_RESOURCE and any
      element in the document is: 
      1. Unclassified and meets ISM_CONTRIBUTES 
      AND 
      2. Has the attribute @ism:nonICmarkings containing [SBU-NF] 
      AND
      3. The ISM_RESOURCE_ELEMENT has attribute @ism:nonICmarkings does not contain [XD] or [ND] 
      AND
      4. The ISM_RESOURCE_ELEMENT has attribute @ism:disseminationControls does not contain [NF]
      Then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [SBU-NF]. 
      
      Human Readable: USA Unclassified documents having SBU-NF and not having XD, ND, or explicit Foreign Disclosure and
      Release markings must have SBU-NF at the resource level.
    </sch:assert>
  </sch:rule>
</sch:pattern>