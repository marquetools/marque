<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00105">
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    [ISM-ID-00105][Error] If the document is an ISM_USGOV_RESOURCE and any
    element in the document is: 
    1. Unclassifed and meets ISM_CONTRIBUTES 
      AND 
    2. Has the attribute @ism:nonICmarkings containing [SBU] 
      AND 
    3. No element meeting ISM_CONTRIBUTES in the document has @ism:nonICmarkings containing any of [SBU-NF], 
       [XD], or [ND]
    Then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [SBU]. 
    
    Human Readable: USA Unclassified documents having SBU and not having SBU-NF, XD, or ND must have SBU at the resource level. 
  </sch:p>
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    If the document is Unclassified and is an ISM_USGOV_RESOURCE, the current
    element is the ISM_RESOURCE_ELEMENT, some element meeting ISM_CONTIBUTES specifies attribute
    @ism:nonICmarkings with a value containing the token [SBU], and no element meeting
    ISM_CONTRIBUTES specifies attribute @ism:nonICmarkings with a value containing the token
    [SBU-NF], [XD], and [ND], then this rule ensures that ISM_RESOURCE_ELEMENT sepcifies attribute
    @ism:nonICmarkings with a value containing the token [SBU]. 
  </sch:p>
  <sch:rule id="ISM-ID-00105-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and $bannerClassification = 'U' and index-of($partNonICmarkings_tok, 'SBU') &gt; 0 and not(util:containsAnyOfTheTokens(string-join($partNonICmarkings, ' '), ('SBU-NF', 'XD', 'ND')))]">
    <sch:assert test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('SBU'))" flag="error" role="error">
      [ISM-ID-00105][Error] If the document is an ISM_USGOV_RESOURCE and any
      element in the document is: 
      1. Unclassifed and meets ISM_CONTRIBUTES 
      AND 
      2. Has the attribute @ism:nonICmarkings containing [SBU] 
      AND 
      3. No element meeting ISM_CONTRIBUTES in the document has @ism:nonICmarkings containing any of [SBU-NF], 
      [XD], or [ND]
      Then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [SBU]. 
      
      Human Readable: USA Unclassified documents having SBU and not having SBU-NF, XD, or ND must have SBU at the resource level.
    </sch:assert>
  </sch:rule>
</sch:pattern>