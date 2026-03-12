<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00150">
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    [ISM-ID-00150][Error] If (ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE) and:
    1. Any element, other than ISM_RESOURCE_ELEMENT, meeting ISM_CONTRIBUTES in the document has the 
    attribute @ism:nonICmarkings containing [LES] or the attribute @ism:cuiBasic containing [LEI]
    AND
    2. No element meeting ISM_CONTRIBUTES in the document has the attribute @ism:noticeType containing [LES]
    
    Human Readable: USA documents containing LES non-IC markings or LEI cuiBasic markings must also have an 
    LES notice.
  </sch:p>
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    If the document is an ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, for each element which
    is not the ISM_RESOURCE_ELEMENT and meets ISM_CONTRIBUTES and specifies 
    attribute @ism:nonICmarkings with a value containing the token [LES]
    or @ism:cuiBasic with a value containing the token [LEI], 
    this rule ensures that an element meeting ISM_CONTRIBUTES specifies attribute
    @ism:noticeType with a value containing the token [LES].
  </sch:p>
  <sch:rule id="ISM-ID-00150-R1" context="*[($ISM_USGOV_RESOURCE or $ISM_USCUIONLY_RESOURCE) and not(generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)) and (util:containsAnyOfTheTokens(@ism:nonICmarkings, ('LES')) or util:containsAnyOfTheTokens(@ism:cuiBasic, ('LEI')))]">
      <sch:assert test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('LES')) and not ($elem/@ism:externalNotice=true()))" flag="error" role="error">
        [ISM-ID-00150][Error] If (ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE) and:
        1. Any element, other than ISM_RESOURCE_ELEMENT, meeting ISM_CONTRIBUTES in the document has the 
        attribute @ism:nonICmarkings containing [LES] or the attribute @ism:cuiBasic containing [LEI]
        AND
        2. No element meeting ISM_CONTRIBUTES in the document has the attribute @ism:noticeType containing [LES]
        
        Human Readable: USA documents containing LES non-IC markings or LEI cuiBasic markings must also have an 
        LES notice.
    </sch:assert>
  </sch:rule>
</sch:pattern>