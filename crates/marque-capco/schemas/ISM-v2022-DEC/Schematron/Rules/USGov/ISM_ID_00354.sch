<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00354" is-a="NtkHasCorrespondingData">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00354][Error] The banner or a portion that contributes to roll-up must contain [XD]
      if EXDIS NTK metadata exists.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      Invokes abstract rule NtkHasCorrespondingData.
   </sch:p>
   <sch:param name="ruleId" value="'ISM-ID-00354'"/>
   <sch:param name="policyName" value="'EXDIS'"/>
   <sch:param name="uriPrefix" value="'urn:us:gov:ic:aces:ntk:xd'"/>
   <sch:param name="attr" value="'nonICmarkings'"/>
   <sch:param name="dataType" value="'XD'"/>
   <sch:param name="dataTokenList" value="$partNonICmarkings_tok"/>
   <sch:param name="bannerTokenList" value="$bannerNonICmarkings_tok"/>
</sch:pattern>