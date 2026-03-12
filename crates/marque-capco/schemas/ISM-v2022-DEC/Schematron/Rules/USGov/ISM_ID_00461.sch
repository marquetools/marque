<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00461">
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText"> 
    [ISM-ID-00461][Error] If ISM_USDOD_RESOURCE and 
    1. not ISM_DOD_DISTRO_EXEMPT
    AND 
    2. Attribute @ism:noticeType of any portion that is not @ism:excludeFromRollup="true" contains [ITAR-EAR],
    then there must be @ism:noticeType=[ITAR-EAR] on the resource element. 
    
    Human Readable: All US DOD documents that do not claim exemption from DoD5230.24 and that have an [ITAR-EAR] notice
    on any portion must have an [ITAR-EAR] notice on the resource element. 
  </sch:p>
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    If the document is an ISM_USDOD_RESOURCE and not ISM_DOD_DISTRO_EXEMPT and has any portion with @ism:noticeType=[ITAR-EAR], and
    the current element is the ISM_RESOURCE_ELEMENT, this rule ensures that attribute @ism:noticeType is
    specified on the resource element with a value of [ITAR-EAR].
  </sch:p>
  <sch:rule id="ISM-ID-00461-R1" context="*[$ISM_USDOD_RESOURCE and not($ISM_DOD_DISTRO_EXEMPT) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (count($partNoticeType_tok[.='ITAR-EAR'])&gt;0)]">
    <sch:assert test="index-of(tokenize(@ism:noticeType,' '), 'ITAR-EAR') &gt; 0" flag="error" role="error">
      [ISM-ID-00461][Error] If ISM_USDOD_RESOURCE and 
      1. not ISM_DOD_DISTRO_EXEMPT
      AND 
      2. Attribute @ism:noticeType of any portion that is not @ism:excludeFromRollup="true" contains [ITAR-EAR],
      then there must be @ism:noticeType=[ITAR-EAR] on the resource element. 
      
      Human Readable: All US DOD documents that do not claim exemption from DoD5230.24 and that have an [ITAR-EAR] notice
      on any portion must have an [ITAR-EAR] notice on the resource element. 
    </sch:assert>
  </sch:rule>
</sch:pattern>
