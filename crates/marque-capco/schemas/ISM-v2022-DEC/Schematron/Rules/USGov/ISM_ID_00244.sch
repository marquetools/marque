<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00244">
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    [ISM-ID-00244][Error] If ISM_USGOV_RESOURCE and:
    1. Any element meeting ISM_CONTRIBUTES in the document has the attribute @ism:atomicEnergyMarkings containing [RD-CNWDI]
    AND
    2. No element meeting ISM_CONTRIBUTES in the document has @ism:noticeType containing [CNWDI].
    that does not have attribute @ism:externalNotice with a value of [true].
    
    Human Readable: USA documents containing CNWDI data must also have an CNWDI notice.
  </sch:p>
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    If the document is an ISM_USGOV_RESOURCE, for each element meeting
    ISM_CONTRIBUTES which specifies attribute @ism:atomicEnergyMarkings with
    a value containing the token [RD-CNWDI], then this rule ensures that some element
    in the document specifies attribute @ism:noticeType with a value containing
    the token [CNWDI] and not an attribute @ism:externalNotice with a value of [true].
  </sch:p>
  <sch:rule id="ISM-ID-00244-R1" context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD-CNWDI'))]">
      <sch:assert test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('CNWDI')) and not ($elem/@ism:externalNotice=true()))" flag="error" role="error">
        [ISM-ID-00244][Error] If ISM_USGOV_RESOURCE and:
        1. Any element meeting ISM_CONTRIBUTES in the document has the attribute @ism:atomicEnergyMarkings containing [RD-CNWDI]
        AND
        2. No element meeting ISM_CONTRIBUTES in the document has @ism:noticeType containing [CNWDI].
        that does not have attribute @ism:externalNotice with a value of [true].
        
        Human Readable: USA documents containing CNWDI data must also have an CNWDI notice.
    </sch:assert>
  </sch:rule>
</sch:pattern>